use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct Task {
    id: usize,
    priority: Option<char>,
    text: String,
    completed: bool,
}

enum Command {
    Add(Option<char>, String),
    List,
    Complete(usize),
    Delete(usize),
}

fn main() {
    #[cfg(windows)]
    enable_ansi_support();
    
    if let Err(e) = run() {
        print!("\x1b[38;2;255;50;50mError\x1b[0m\x1b[38;2;255;255;255m:\x1b[0m ");
        println!("\x1b[38;2;255;255;255m{}\x1b[0m", e);
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn enable_ansi_support() {
    use windows::Win32::System::Console::{
        GetStdHandle, GetConsoleMode, SetConsoleMode,
        CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };
    
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE).unwrap();
        let mut mode = CONSOLE_MODE(0);
        let _ = GetConsoleMode(handle, &mut mode);
        let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = parse_command(&args)?;
    let todo_path = find_todo_file()?;
    
    match cmd {
        Command::Add(priority, text) => add_task(&todo_path, priority, text)?,
        Command::List => list_tasks(&todo_path)?,
        Command::Complete(id) => complete_task(&todo_path, id)?,
        Command::Delete(id) => delete_task(&todo_path, id)?,
    }
    
    Ok(())
}

fn parse_command(args: &[String]) -> io::Result<Command> {
    if args.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "No command provided"));
    }
    
    match args[0].as_str() {
        "list" | "ls" | "l" => Ok(Command::List),
        "d" | "do" if args.len() > 1 => {
            let id = args[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid task ID"))?;
            Ok(Command::Complete(id))
        }
        "del" | "delete" if args.len() > 1 => {
            let id = args[1].parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid task ID"))?;
            Ok(Command::Delete(id))
        }
        first if first.len() == 1 && matches!(first.to_uppercase().as_str(), "A" | "B" | "C") => {
            Ok(Command::Add(first.to_uppercase().chars().next(), args[1..].join(" ")))
        }
        _ => {
            if args.len() == 1 && args[0].len() == 1 && args[0].chars().next().unwrap().is_ascii_alphabetic() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid command"));
            }
            Ok(Command::Add(None, args.join(" ")))
        }
    }
}

fn find_todo_file() -> io::Result<PathBuf> {
    let current_dir = env::current_dir()?;
    let local_todo = current_dir.join("todo.txt");
    
    if local_todo.exists() {
        return Ok(local_todo);
    }
    
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))?;
    
    Ok(exe_dir.join("todo.txt"))
}

fn read_tasks(path: &PathBuf) -> io::Result<Vec<Task>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    
    let content = fs::read_to_string(path)?;
    let tasks: Vec<Task> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| parse_task(idx + 1, line))
        .collect();
    
    Ok(tasks)
}

fn parse_task(id: usize, line: &str) -> Task {
    let line = line.trim();
    
    if line.is_empty() {
        return Task { id, priority: None, text: String::new(), completed: false };
    }
    
    let completed = line.starts_with("x ");
    let line = if completed { 
        if line.len() > 2 { 
            line[2..].trim() 
        } else { 
            "" 
        }
    } else { 
        line 
    };
    
    let (priority, text) = if line.len() > 3 
        && line.starts_with('(') 
        && line.chars().nth(2) == Some(')') 
        && line.chars().nth(1).map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        let p = line.chars().nth(1).unwrap().to_ascii_uppercase();
        let t = if line.len() > 4 { line[4..].to_string() } else { String::new() };
        (Some(p), t)
    } else {
        (None, line.to_string())
    };
    
    Task { id, priority, text, completed }
}

fn priority_order(p: &Option<char>) -> u8 {
    match p {
        Some('A') => 0,
        Some('B') => 1,
        Some('C') => 2,
        _ => 3,
    }
}

fn format_priority_color(priority: Option<char>) -> String {
    match priority {
        Some('A') => format!("\x1b[38;2;255;50;50m[A]\x1b[0m"),
        Some('B') => format!("\x1b[38;2;255;200;0m[B]\x1b[0m"),
        Some('C') => format!("\x1b[38;2;50;200;50m[C]\x1b[0m"),
        _ => String::new(),
    }
}

fn format_action(action: &str, priority: Option<char>, text: &str, id: usize) {
    print!("\x1b[38;2;50;200;50m{}\x1b[0m\x1b[38;2;255;255;255m:\x1b[0m ", action);
    if let Some(p) = priority {
        print!("{} ", format_priority_color(Some(p)));
    }
    print!("\x1b[38;2;255;255;255m{}\x1b[0m ", text);
    println!("\x1b[38;2;120;120;120m(№{})\x1b[0m", id);
}

fn add_task(path: &PathBuf, priority: Option<char>, text: String) -> io::Result<()> {
    let line = match priority {
        Some(p) => format!("({}) {}", p.to_ascii_uppercase(), text),
        None => text.clone(),
    };
    
    let mut lines = if path.exists() {
        fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    
    let pos = lines.iter().position(|l| {
        priority_order(&parse_priority(l)) > priority_order(&priority)
    }).unwrap_or(lines.len());
    
    lines.insert(pos, line);
    fs::write(path, lines.join("\n") + "\n")?;
    
    let tasks = read_tasks(path)?;
    let id = tasks.iter()
        .find(|t| t.priority == priority && t.text == text && !t.completed)
        .map(|t| t.id)
        .unwrap_or(pos + 1);
    
    format_action("Added", priority, &text, id);
    Ok(())
}

fn parse_priority(line: &str) -> Option<char> {
    let line = line.trim();
    let line = if line.starts_with("x ") && line.len() > 2 {
        line[2..].trim()
    } else {
        line
    };
    
    if line.len() > 3 
        && line.starts_with('(') 
        && line.chars().nth(2) == Some(')') 
        && line.chars().nth(1).map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        Some(line.chars().nth(1).unwrap().to_ascii_uppercase())
    } else {
        None
    }
}

fn list_tasks(path: &PathBuf) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let mut all_tasks: Vec<&Task> = tasks.iter().collect();
    
    if all_tasks.is_empty() {
        println!("\x1b[38;2;255;255;255mNo tasks.\x1b[0m");
        println!();
        println!("\x1b[38;2;255;255;255mYou can add new tasks with `todo B text`\x1b[0m");
        return Ok(());
    }
    
    all_tasks.sort_by(|a, b| {
        priority_order(&a.priority).cmp(&priority_order(&b.priority)).then(a.id.cmp(&b.id))
    });
    
    let max_id = all_tasks.iter().map(|t| t.id).max().unwrap_or(0);
    let width = max_id.to_string().len();
    
    for task in all_tasks {
        let (pri_color, id_color, text_color) = if task.completed {
            match task.priority {
                Some(_) => ("\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m"),
                None => ("", "\x1b[38;2;5;155;5m", "\x1b[38;2;5;155;5m"),
            }
        } else {
            match task.priority {
                Some('A') => ("\x1b[38;2;255;50;50m", "\x1b[38;2;130;130;130m", "\x1b[38;2;255;255;255m"),
                Some('B') => ("\x1b[38;2;255;200;0m", "\x1b[38;2;130;130;130m", "\x1b[38;2;255;255;255m"),
                Some('C') => ("\x1b[38;2;50;200;50m", "\x1b[38;2;130;130;130m", "\x1b[38;2;255;255;255m"),
                Some(_) | None => ("", "\x1b[38;2;130;130;130m", "\x1b[38;2;210;210;210m"),
            }
        };
        
        match task.priority {
            Some(p) => {
                print!("{}\x1b[1m[{}]\x1b[0m", pri_color, p);
                print!(" {}{:>width$}\x1b[0m", id_color, task.id, width = width);
                println!(" {}{}\x1b[0m", text_color, task.text);
            }
            None => {
                print!("    ");
                print!("{}{:>width$}\x1b[0m", id_color, task.id, width = width);
                println!(" {}{}\x1b[0m", text_color, task.text);
            }
        }
    }
    
    Ok(())
}

fn complete_task(path: &PathBuf, id: usize) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let task = tasks.iter().find(|t| t.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Task not found"))?;
    
    if task.completed {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Task already completed"));
    }
    
    let (priority, text) = (task.priority, task.text.clone());
    
    let lines: Vec<String> = fs::read_to_string(path)?
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim();
            if line.is_empty() { return None; }
            Some(if idx + 1 == id { format!("x {}", line) } else { line.to_string() })
        })
        .collect();
    
    fs::write(path, lines.join("\n") + "\n")?;
    format_action("Completed", priority, &text, id);
    Ok(())
}

fn delete_task(path: &PathBuf, id: usize) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let task = tasks.iter()
        .find(|t| t.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Task not found"))?;
    
    let (priority, text) = (task.priority, task.text.clone());
    
    let lines: Vec<String> = fs::read_to_string(path)?
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim();
            if line.is_empty() || idx + 1 == id { None } else { Some(line.to_string()) }
        })
        .collect();
    
    fs::write(path, lines.join("\n") + "\n")?;
    format_action("Deleted", priority, &text, id);
    Ok(())
}