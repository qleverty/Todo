use std::env;
use std::fs;
use std::io::{self, Write};
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
        eprintln!("Error: {}", e);
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
        first if first.len() == 1 && first.chars().next().unwrap().is_ascii_alphabetic() => {
            let priority = first.to_uppercase().chars().next();
            let text = args[1..].join(" ");
            Ok(Command::Add(priority, text))
        }
        _ => {
            let text = args.join(" ");
            Ok(Command::Add(None, text))
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

fn add_task(path: &PathBuf, priority: Option<char>, text: String) -> io::Result<()> {
    let new_task_line = if let Some(p) = priority {
        format!("({}) {}", p.to_ascii_uppercase(), text)
    } else {
        text.clone()
    };
    
    if !path.exists() {
        fs::write(path, format!("{}\n", new_task_line))?;
        println!("Added: {}", text);
        return Ok(());
    }
    
    let content = fs::read_to_string(path)?;
    let mut lines: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|s| s.to_string())
        .collect();
    
    let new_priority_order = priority_order(&priority);
    let mut insert_pos = lines.len();
    
    for (idx, line) in lines.iter().enumerate() {
        let line_priority = parse_priority(line);
        let line_priority_order = priority_order(&line_priority);
        
        if line_priority_order > new_priority_order {
            insert_pos = idx;
            break;
        } else if line_priority_order == new_priority_order {
            insert_pos = idx;
            break;
        }
    }
    
    lines.insert(insert_pos, new_task_line);
    fs::write(path, lines.join("\n") + "\n")?;
    println!("Added: {}", text);
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
    let mut active_tasks: Vec<&Task> = tasks.iter().filter(|t| !t.completed).collect();
    
    if active_tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }
    
    active_tasks.sort_by(|a, b| {
        let a_order = priority_order(&a.priority);
        let b_order = priority_order(&b.priority);
        a_order.cmp(&b_order).then(a.id.cmp(&b.id))
    });
    
    for task in active_tasks {
        match task.priority {
            Some('A') => {
                print!("\x1b[38;2;255;50;50m\x1b[1m[A]\x1b[0m");
                print!(" \x1b[38;2;150;150;150m{}\x1b[0m", task.id);
                println!("  \x1b[38;2;255;255;255m{}\x1b[0m", task.text);
            }
            Some('B') => {
                print!("\x1b[38;2;255;200;0m\x1b[1m[B]\x1b[0m");
                print!(" \x1b[38;2;150;150;150m{}\x1b[0m", task.id);
                println!("  \x1b[38;2;255;255;255m{}\x1b[0m", task.text);
            }
            Some('C') => {
                print!("\x1b[38;2;50;200;50m\x1b[1m[C]\x1b[0m");
                print!(" \x1b[38;2;150;150;150m{}\x1b[0m", task.id);
                println!("  \x1b[38;2;255;255;255m{}\x1b[0m", task.text);
            }
            _ => {
                print!("    ");
                print!("\x1b[38;2;150;150;150m{}\x1b[0m", task.id);
                println!("  \x1b[38;2;200;200;200m{}\x1b[0m", task.text);
            }
        }
    }
    
    Ok(())
}

fn complete_task(path: &PathBuf, id: usize) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let task = tasks.iter()
        .find(|t| t.id == id && !t.completed)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Task not found"))?;
    
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let new_lines: Vec<String> = lines.iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            if idx + 1 == id {
                Some(format!("x {}", line))
            } else {
                Some(line.to_string())
            }
        })
        .collect();
    
    fs::write(path, new_lines.join("\n") + "\n")?;
    println!("Completed: {}", task.text);
    Ok(())
}

fn delete_task(path: &PathBuf, id: usize) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let task = tasks.iter()
        .find(|t| t.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Task not found"))?;
    
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let new_lines: Vec<String> = lines.iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim();
            if line.is_empty() || idx + 1 == id {
                None
            } else {
                Some(line.to_string())
            }
        })
        .collect();
    
    fs::write(path, new_lines.join("\n") + "\n")?;
    println!("Deleted: {}", task.text);
    Ok(())
}