use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
use std::path::PathBuf;
use std::collections::HashSet;
use std::time::Duration;
use serde::Deserialize;

#[derive(Debug, Clone)]
struct Task {
    id: usize,
    priority: Option<char>,
    text: String,
    completed: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

struct UpdateInfo {
    current_version: String,
    latest_version: String,
    download_url: String,
}

enum Command {
    Add(Option<char>, String),
    List,
    Complete(Vec<usize>),
    Delete(Vec<usize>),
    Edit(usize, Option<char>, Option<String>),
    Clear,
    Help,
    Update,
    Rollback,
    Resort,
}

fn confirm_action(prompt: &str) -> io::Result<bool> {
    use io::Write;
    
    loop {
        print!("\x1b[38;2;255;255;255m{}\x1b[0m", prompt);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().to_lowercase().as_str() {
            "y" => return Ok(true),
            "n" => return Ok(false),
            _ => continue,
        }
    }
}

fn main() {
    #[cfg(windows)]
    enable_ansi_support();
    
    check_update_state();
    
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
    
    match cmd {
        Command::Update => return handle_update(),
        Command::Rollback => return handle_rollback(),
        _ => {}
    }
    
    let todo_path = find_todo_file()?;
    
    match cmd {
        Command::Add(priority, text) => add_task(&todo_path, priority, text)?,
        Command::List => list_tasks(&todo_path)?,
        Command::Complete(id) => complete_task(&todo_path, id)?,
        Command::Delete(id) => delete_task(&todo_path, id)?,
        Command::Edit(id, priority, text) => edit_task(&todo_path, id, priority, text)?,
        Command::Clear => clear_completed(&todo_path)?,
        Command::Help => show_help(),
        Command::Update | Command::Rollback => unreachable!(),
		Command::Resort => resort_tasks(&todo_path)?,
    }
    
    Ok(())
}

fn is_valid_batch_arg(s: &str) -> bool {
    if s.is_empty() || s.starts_with('-') || s.ends_with('-') {
        return false;
    }
    
    let dash_count = s.chars().filter(|&c| c == '-').count();
    if dash_count > 1 {
        return false;
    }
    
    s.chars().all(|c| c.is_ascii_digit() || c == '-')
}

fn parse_batch_args(args: &[String]) -> Result<Vec<usize>, String> {
    if !args.iter().all(|arg| is_valid_batch_arg(arg)) {
        return Err("Invalid batch format".to_string());
    }
    
    let mut ids = HashSet::new();
    
    for arg in args {
        if let Some(pos) = arg.find('-') {
            let start: usize = arg[..pos].parse().map_err(|_| "Invalid range")?;
            let end: usize = arg[pos + 1..].parse().map_err(|_| "Invalid range")?;
            
            if start > end {
                return Err("Invalid range".to_string());
            }
            
            for id in start..=end {
                ids.insert(id);
            }
        } else {
            let id: usize = arg.parse().map_err(|_| "Invalid ID")?;
            ids.insert(id);
        }
    }
    
    Ok(ids.into_iter().collect())
}

fn parse_command(args: &[String]) -> io::Result<Command> {
    if args.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "No command provided"));
    }
    
    match args[0].as_str() {
        "list" | "ls" | "l" if args.len() == 1 => Ok(Command::List),
        "clear" | "clr" if args.len() == 1 => Ok(Command::Clear),
        "help" | "h" if args.len() == 1 => Ok(Command::Help),
        "update" if args.len() == 1 => Ok(Command::Update),
        "rollback" if args.len() == 1 => Ok(Command::Rollback),
		"resort" if args.len() == 1 => Ok(Command::Resort),
		"edit" | "e" => {
			if args.len() < 2 {
				return Err(io::Error::new(io::ErrorKind::InvalidInput, "No task ID provided."));
			}
			let id = args[1].parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid task ID"))?;
			if args.len() == 2 {
				return Ok(Command::Edit(id, None, None));
			}
			let first = &args[2];
			let (priority, text_start) = if first == "-" {
				(None, 3)
			} else if first.len() == 1 && matches!(first.to_uppercase().as_str(), "A" | "B" | "C") {
				(first.to_uppercase().chars().next(), 3)
			} else {
				(None, 2)
			};
			let text = if args.len() > text_start { Some(args[text_start..].join(" ")) } else { None };
			Ok(Command::Edit(id, priority, text))
		},
        "d" | "do" => {
            if args.len() == 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "No task ID provided.\nExample: todo d 5"));
            }
            match parse_batch_args(&args[1..]) {
                Ok(ids) => Ok(Command::Complete(ids)),
                Err(_) => Ok(Command::Add(None, args.join(" "))),
            }
        },
        "del" | "delete" => {
            if args.len() == 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "No task ID provided.\nExample: todo delete 5"));
            }
            match parse_batch_args(&args[1..]) {
                Ok(ids) => Ok(Command::Delete(ids)),
                Err(_) => Ok(Command::Add(None, args.join(" "))),
            }
        },
        first if first.len() == 1 && matches!(first.to_uppercase().as_str(), "A" | "B" | "C") => {
            if args.len() == 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "No text of task provided"));
            }
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

fn priority_order(priority: &Option<char>) -> u8 {
    match priority {
        Some('A') => 0,
        Some('B') => 1,
        Some('C') => 2,
        _ => 3,
    }
}

fn format_action(action: &str, priority: Option<char>, text: &str, id: usize) {
    print!("\x1b[38;2;50;200;50m{}\x1b[0m\x1b[38;2;255;255;255m:\x1b[0m ", action);
    
    if let Some(p) = priority {
        let color = match p {
            'A' => "\x1b[38;2;255;50;50m",
            'B' => "\x1b[38;2;255;200;0m",
            'C' => "\x1b[38;2;50;200;50m",
            _ => "\x1b[38;2;50;200;50m",
        };
        print!("{}\x1b[1m[{}]\x1b[0m ", color, p);
    }
    
    print!("\x1b[38;2;255;255;255m{}\x1b[0m ", text);
    println!("\x1b[38;2;210;210;210m(№{})\x1b[0m", id);
}

fn add_task(path: &PathBuf, priority: Option<char>, text: String) -> io::Result<()> {
    let mut lines: Vec<String> = if path.exists() {
        fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };
    
    let task_line = match priority {
        Some(p) => format!("({}) {}", p, text),
        None => text.clone(),
    };
    
    let priority_order = |p: Option<char>| match p {
        Some('A') => 0,
        Some('B') => 1,
        Some('C') => 2,
        _ => 3,
    };
    
    let new_priority_order = priority_order(priority);
    
    let insert_pos = lines.iter().position(|line| {
        let line_priority = if line.len() > 3 && line.starts_with('(') && line.chars().nth(2) == Some(')') {
            line.chars().nth(1)
        } else {
            None
        };
        priority_order(line_priority) >= new_priority_order
    }).unwrap_or(lines.len());
    
    lines.insert(insert_pos, task_line);
    
    let next_id = insert_pos + 1;
    
    fs::write(path, lines.join("\n") + "\n")?;
    format_action("Added", priority, &text, next_id);
    Ok(())
}

fn list_tasks(path: &PathBuf) -> io::Result<()> {
    let mut all_tasks = read_tasks(path)?;
    
    if all_tasks.is_empty() {
		print!("\x1b[38;2;255;50;50mError\x1b[0m\x1b[38;2;255;255;255m:\x1b[0m ");
		println!("\x1b[38;2;255;255;255mNo tasks\x1b[0m");
		println!("\x1b[38;2;210;210;210mUse `todo help` for command list\x1b[0m");
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
                Some('A') => ("\x1b[38;2;255;50;50m", "\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m"),
                Some('B') => ("\x1b[38;2;255;200;0m", "\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m"),
                Some('C') => ("\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m", "\x1b[38;2;50;200;50m"),
                Some(_) | None => ("", "\x1b[38;2;5;155;5m", "\x1b[38;2;5;155;5m"),
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

fn complete_task(path: &PathBuf, ids: Vec<usize>) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    
    let valid_ids: Vec<usize> = ids.iter()
        .filter(|&&id| tasks.iter().any(|t| t.id == id && !t.completed))
        .copied()
        .collect();
    
    if valid_ids.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No valid tasks found"));
    }
    
    if valid_ids.len() == 1 {
        let id = valid_ids[0];
        let task = tasks.iter().find(|t| t.id == id).unwrap();
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
    } else {
        let valid_set: HashSet<usize> = valid_ids.iter().copied().collect();
        
        let lines: Vec<String> = fs::read_to_string(path)?
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| {
                let line = line.trim();
                if line.is_empty() { return None; }
                Some(if valid_set.contains(&(idx + 1)) { 
                    format!("x {}", line) 
                } else { 
                    line.to_string() 
                })
            })
            .collect();
        
        fs::write(path, lines.join("\n") + "\n")?;
        println!("\x1b[38;2;50;200;50mCompleted\x1b[0m\x1b[38;2;255;255;255m: {} tasks were completed\x1b[0m", valid_ids.len());
    }
    
    Ok(())
}

fn delete_task(path: &PathBuf, ids: Vec<usize>) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    
    let valid_ids: Vec<usize> = ids.iter()
        .filter(|&&id| tasks.iter().any(|t| t.id == id))
        .copied()
        .collect();
    
    if valid_ids.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No valid tasks found"));
    }
    
    if valid_ids.len() == 1 {
        let id = valid_ids[0];
        let task = tasks.iter().find(|t| t.id == id).unwrap();
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
    } else {
        let valid_set: HashSet<usize> = valid_ids.iter().copied().collect();
        
        let lines: Vec<String> = fs::read_to_string(path)?
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| {
                let line = line.trim();
                if line.is_empty() || valid_set.contains(&(idx + 1)) { 
                    None 
                } else { 
                    Some(line.to_string()) 
                }
            })
            .collect();
        
        fs::write(path, lines.join("\n") + "\n")?;
        println!("\x1b[38;2;50;200;50mDeleted\x1b[0m\x1b[38;2;255;255;255m: {} tasks were deleted\x1b[0m", valid_ids.len());
    }
    
    Ok(())
}

fn edit_task(path: &PathBuf, id: usize, new_priority: Option<char>, new_text: Option<String>) -> io::Result<()> {
    let tasks = read_tasks(path)?;
    let task = tasks.iter().find(|t| t.id == id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Task №{} not found", id)))?;
    
    let (old_p, old_t) = (task.priority, task.text.clone());
	let final_p = new_priority;
	let final_t = new_text.as_ref().unwrap_or(&old_t);
    
    let lines: Vec<String> = fs::read_to_string(path)?
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line = line.trim();
            if line.is_empty() { return None; }
            if idx + 1 != id { return Some(line.to_string()); }
            
            let prefix = if task.completed { "x " } else { "" };
            Some(match final_p {
                Some(p) => format!("{}({}) {}", prefix, p, final_t),
                None => format!("{}{}", prefix, final_t),
            })
        })
        .collect();
    
    fs::write(path, lines.join("\n") + "\n")?;
    
	let fmt_p = |p: Option<char>| match p {
		Some('A') => format!("\x1b[38;2;255;50;50m[A]\x1b[0m"),
		Some('B') => format!("\x1b[38;2;255;200;0m[B]\x1b[0m"),
		Some('C') => format!("\x1b[38;2;50;200;50m[C]\x1b[0m"),
		_ => format!("\x1b[38;2;130;130;130m[-]\x1b[0m"),
	};

	let text_color = |p: Option<char>| if p.is_some() { "\x1b[38;2;255;255;255m" } else { "\x1b[38;2;210;210;210m" };

	print!("\x1b[38;2;50;200;50mEdited\x1b[0m\x1b[38;2;255;255;255m:\x1b[0m ");

	if new_text.is_none() {
		println!("\x1b[38;2;255;255;255m№{} priority:\x1b[0m {} \x1b[38;2;210;210;210m→\x1b[0m {}", id, fmt_p(old_p), fmt_p(final_p));
	} else {
		println!("\x1b[38;2;255;255;255m№{}:\x1b[0m {} {}\"{}\"\x1b[0m \x1b[38;2;210;210;210m→\x1b[0m {} {}\"{}\"\x1b[0m", 
			id, fmt_p(old_p), text_color(old_p), old_t, fmt_p(final_p), text_color(final_p), final_t);
	}
    
    Ok(())
}

fn extract_priority(line: &str) -> Option<char> {
    let line = line.trim();
    let line = if line.starts_with("x ") && line.len() > 2 {
        line[2..].trim()
    } else {
        line
    };
    
    if line.len() > 3 && line.starts_with('(') && line.chars().nth(2) == Some(')') {
        line.chars().nth(1).filter(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_uppercase())
    } else {
        None
    }
}

fn resort_tasks(path: &PathBuf) -> io::Result<()> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines()
        .map(|s| s.to_string())
        .filter(|l| !l.trim().is_empty())
        .collect();
    
    if lines.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "File empty"));
    }
    
    let mut groups: [Vec<String>; 4] = Default::default();
    
    for line in lines.iter() {
        let index = match extract_priority(line) {
            Some('A') => 0,
            Some('B') => 1,
            Some('C') => 2,
            _ => 3,
        };
        groups[index].push(line.clone());
    }
    
    let sorted: Vec<String> = groups.into_iter().flatten().collect();
    
    if sorted == lines {
        println!("\x1b[38;2;50;200;50mResorted\x1b[0m\x1b[38;2;255;255;255m: File already resorted\x1b[0m");
        return Ok(());
    }
    
    fs::write(path, sorted.join("\n") + "\n")?;
    println!("\x1b[38;2;50;200;50mResorted\x1b[0m\x1b[38;2;255;255;255m: Successfully resorted todo.txt file\x1b[0m");
    Ok(())
}

fn clear_completed(path: &PathBuf) -> io::Result<()> {
    if !path.exists() {
        println!("\x1b[38;2;50;200;50mCleared\x1b[0m\x1b[38;2;255;255;255m: 0 tasks were deleted\x1b[0m");
        return Ok(());
    }
    
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let completed_count = lines.iter()
        .filter(|line| !line.trim().is_empty() && line.trim().starts_with("x "))
        .count();
    
    if completed_count == 0 {
        println!("\x1b[38;2;50;200;50mCleared\x1b[0m\x1b[38;2;255;255;255m: 0 tasks were deleted\x1b[0m");
        return Ok(());
    }
	
	if !confirm_action("Do you want to install? (y/n) ")? {
		return Ok(());
	}
    
    let filtered: Vec<String> = lines.iter()
        .filter_map(|&line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("x ") {
                None
            } else {
                Some(line.to_string())
            }
        })
        .collect();
    
    fs::write(path, filtered.join("\n") + "\n")?;
    println!("\x1b[38;2;50;200;50mCleared\x1b[0m\x1b[38;2;255;255;255m: {} tasks were deleted\x1b[0m", completed_count);
    Ok(())
}

fn handle_update() -> io::Result<()> {
    println!("\x1b[38;2;255;255;255mChecking for updates...\x1b[0m");
    
    match check_for_update() {
        Ok(Some(info)) => {
            println!("\x1b[38;2;50;200;50mNew version available!\x1b[0m");
            println!("\x1b[38;2;255;255;255mCurrent: v{}\x1b[0m", info.current_version);
            println!("\x1b[38;2;255;255;255mLatest: v{}\x1b[0m", info.latest_version);
            println!();
            println!("\x1b[38;2;255;255;255mDownloading update...\x1b[0m");
            
            let zip_data = download_update(&info.download_url)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            
            println!("\x1b[38;2;255;255;255mExtracting...\x1b[0m");
            extract_update(&zip_data)?;
            
            println!("\x1b[38;2;255;255;255mCreating backup...\x1b[0m");
            create_backup()?;
            
            let exe_dir = env::current_exe()?
                .parent()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))?
                .to_path_buf();
            
            #[cfg(windows)]
            let updater_name = "updater.exe";
            #[cfg(not(windows))]
            let updater_name = "updater";
            
            let updater_path = exe_dir.join(updater_name);
            
            if !updater_path.exists() {
                return Err(io::Error::new(io::ErrorKind::NotFound, "Updater not found in downloaded package"));
            }
            
            println!("\x1b[38;2;255;255;255mInstalling update...\x1b[0m");
            
            std::process::Command::new(&updater_path)
                .spawn()?;
            
            std::process::exit(0);
        }
		Ok(None) => {
            println!("\x1b[38;2;50;200;50mУже на последней версии (v{}).\x1b[0m", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Err(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, e));
        }
    }
}

fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    
    let client = reqwest::blocking::Client::builder()
        .user_agent("todo-cli")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get("https://api.github.com/repos/qleverty/todo/releases/latest")
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "Request timed out. Check your internet.".to_string()
            } else if e.is_connect() {
                "Cannot connect to GitHub. Check your internet.".to_string()
            } else {
                format!("Request failed: {}", e)
            }
        })?;
    
    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    
    if current_version == latest_version {
        return Ok(None);
    }
    
    let asset_name = format!("todo-v{}-win64.zip", latest_version);
    let asset = release.assets.iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("Release asset '{}' not found", asset_name))?;
    
	Ok(Some(UpdateInfo {
        current_version,
        latest_version,
        download_url: asset.browser_download_url.clone(),
    }))
}

fn download_update(url: &str) -> io::Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Client error: {}", e)))?;
    
    let response = client.get(url)
        .send()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Download failed: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(io::Error::new(io::ErrorKind::Other, 
            format!("Download failed with status: {}", response.status())));
    }
    
    let bytes = response.bytes()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to read response: {}", e)))?;
    
    Ok(bytes.to_vec())
}

fn extract_update(zip_bytes: &[u8]) -> io::Result<()> {
    let exe_dir = env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))?
        .to_path_buf();
    
    let temp_dir = exe_dir.join("update_temp");
    
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir(&temp_dir)?;
    
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid ZIP: {}", e)))?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("ZIP error: {}", e)))?;
        
        let outpath = match file.enclosed_name() {
            Some(path) => temp_dir.join(path),
            None => continue,
        };
        
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    
    Ok(())
}

fn create_backup() -> io::Result<()> {
    let exe_dir = env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))?
        .to_path_buf();
    
    let temp_dir = exe_dir.join("update_temp");
    let backup_dir = exe_dir.join("update_backup");
    
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }
    fs::create_dir(&backup_dir)?;
    
    let entries = fs::read_dir(&temp_dir)?;
    
    for entry in entries {
        let entry = entry?;
        let filename = entry.file_name();
        let source = exe_dir.join(&filename);
        
        if source.exists() && source.is_file() {
            let dest = backup_dir.join(&filename);
            fs::copy(&source, &dest)?;
        }
    }
    
    Ok(())
}

fn check_update_state() {
	// TODO: Show this message only once after update, not on every startup
    // let exe_dir = match env::current_exe()
    //     .and_then(|p| p.parent()
    //         .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))
    //         .map(|p| p.to_path_buf())) {
    //     Ok(dir) => dir,
    //     Err(_) => return,
    // };
    
    // let temp_exists = exe_dir.join("update_temp").exists();
    // let backup_exists = exe_dir.join("update_backup").exists();
    
    // if !temp_exists && backup_exists {
    //     println!("\x1b[38;2;50;200;50mSuccessfully updated to v{}!\x1b[0m\n", 
    //         env!("CARGO_PKG_VERSION"));
    // }
    
    // if temp_exists && backup_exists {
    //     eprintln!("\x1b[38;2;255;50;50m[!] Previous update failed!\x1b[0m");
    //     eprintln!("\x1b[38;2;255;255;255mRun '\x1b[38;2;50;200;50mtodo rollback\x1b[0m\x1b[38;2;255;255;255m' to restore previous version.\x1b[0m");
    //     eprintln!();
    // } else if temp_exists && !backup_exists {
    //     eprintln!("\x1b[38;2;255;200;0m[!] Incomplete update found.\x1b[0m");
    //     eprintln!("\x1b[38;2;255;255;255mDelete 'update_temp' manually.\x1b[0m");
    //     eprintln!();
    // }
}

fn handle_rollback() -> io::Result<()> {
    let exe_dir = env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find exe directory"))?
        .to_path_buf();
    
    let backup_dir = exe_dir.join("update_backup");
    
    if !backup_dir.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, 
            "No backup found. Nothing to rollback."));
    }
    
    #[cfg(windows)]
    let updater_name = "updater.exe";
    #[cfg(not(windows))]
    let updater_name = "updater";
    
    let updater_path = exe_dir.join(updater_name);
    
    if !updater_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Updater not found"));
    }
    
    println!("\x1b[38;2;255;255;255mRolling back to previous version...\x1b[0m");
    
    std::process::Command::new(&updater_path)
        .arg("rollback")
        .spawn()?;
    
    std::process::exit(0);
}

fn show_help() {
    println!("\x1b[38;2;255;255;255mADD TASKS:\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo some task      → add task without priority\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;50;50mA\x1b[0m\x1b[38;2;210;210;210m urgent task  → add with \x1b[38;2;255;50;50mhigh\x1b[0m\x1b[38;2;210;210;210m priority\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;200;0mB\x1b[0m\x1b[38;2;210;210;210m normal task  → add with \x1b[38;2;255;200;0mmedium\x1b[0m\x1b[38;2;210;210;210m priority\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;50;200;50mC\x1b[0m\x1b[38;2;210;210;210m minor task   → add with \x1b[38;2;50;200;50mlow\x1b[0m\x1b[38;2;210;210;210m priority\x1b[0m");
    println!();
    println!("\x1b[38;2;255;255;255mVIEW TASKS:\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mlist\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255mls\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255ml\x1b[0m\x1b[38;2;210;210;210m      → show all tasks\x1b[0m");
    println!();
    println!("\x1b[38;2;255;255;255mCOMPLETE TASKS:\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdo\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255md\x1b[0m\x1b[38;2;210;210;210m <id|range>... → mark task(s) as completed\x1b[0m");
    println!("  \x1b[38;2;210;210;210mExamples:\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdo\x1b[0m\x1b[38;2;210;210;210m 3         → complete task №3\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdo\x1b[0m\x1b[38;2;210;210;210m 1 5 9     → complete tasks №1, №5, №9\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdo\x1b[0m\x1b[38;2;210;210;210m 4-7       → complete tasks №4, №5, №6, №7 (range)\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdo\x1b[0m\x1b[38;2;210;210;210m 1 4-7 10  → complete №1, №4-7, №10\x1b[0m");
    println!();
    println!("\x1b[38;2;255;255;255mDELETE TASKS:\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdelete\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255mdel\x1b[0m\x1b[38;2;210;210;210m <id|range>... → delete task(s)\x1b[0m");
    println!("  \x1b[38;2;210;210;210mExamples:\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdelete\x1b[0m\x1b[38;2;210;210;210m 5     → delete task №5\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mdelete\x1b[0m\x1b[38;2;210;210;210m 1-3 8 → delete tasks №1, №2, №3, №8\x1b[0m");
	println!();
    println!("\x1b[38;2;255;255;255mEDIT TASKS:\x1b[0m");
    println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255medit\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255me\x1b[0m\x1b[38;2;210;210;210m <id> [priority] [text] → edit task\x1b[0m");
    println!("  \x1b[38;2;210;210;210mExamples:\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255medit\x1b[0m\x1b[38;2;210;210;210m 5 B new text  → change priority to B and text\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255medit\x1b[0m\x1b[38;2;210;210;210m 5 B           → change only priority to B\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255medit\x1b[0m\x1b[38;2;210;210;210m 5 new text    → remove priority and change text\x1b[0m");
    println!("    \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255medit\x1b[0m\x1b[38;2;210;210;210m 5             → remove priority\x1b[0m");
    println!();
    println!("\x1b[38;2;255;255;255mOTHER:\x1b[0m");
	println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mclear\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255mclr\x1b[0m\x1b[38;2;210;210;210m     → remove all completed tasks\x1b[0m");
	println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mresort\x1b[0m\x1b[38;2;210;210;210m        → resort tasks in todo.txt file\x1b[0m");
	println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mupdate\x1b[0m\x1b[38;2;210;210;210m        → check for updates\x1b[0m");
	println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mrollback\x1b[0m\x1b[38;2;210;210;210m      → restore previous version\x1b[0m");
	println!("  \x1b[38;2;210;210;210mtodo \x1b[38;2;255;255;255mhelp\x1b[0m\x1b[38;2;210;210;210m/\x1b[38;2;255;255;255mh\x1b[0m\x1b[38;2;210;210;210m        → show this help\x1b[0m");
    println!();
    println!();
    println!("\x1b[38;2;255;255;255mTO\x1b[38;2;153;229;80mDO\x1b[0m \x1b[38;2;255;255;255mv{}\x1b[0m", env!("CARGO_PKG_VERSION"));
    println!("\x1b[38;2;210;210;210mGitHub: https://github.com/qleverty/todo\x1b[0m");
}