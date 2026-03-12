use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn main() {
    #[cfg(windows)]
    enable_ansi_support();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        error_and_wait("Usage: updater.exe [install|rollback]");
        return;
    }
    
    match args[1].as_str() {
        "install" => install_update(),
        "rollback" => rollback_update(),
        _ => error_and_wait(&format!("Unknown command: {}", args[1])),
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

fn install_update() {
    println!("\nApplying update...\n");
    
    thread::sleep(Duration::from_millis(300));
    
    let exe_dir = match get_exe_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error_and_wait(&format!("Cannot find exe directory: {}", e));
            return;
        }
    };
    
    let temp_dir = exe_dir.join("update_temp");
    
    if !temp_dir.exists() {
        error_and_wait("update_temp not found");
        return;
    }
    
    let entries = match fs::read_dir(&temp_dir) {
        Ok(entries) => entries,
        Err(e) => {
            error_and_wait(&format!("Cannot read update_temp: {}", e));
            return;
        }
    };
    
    let mut all_ok = true;
    
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        if !entry.path().is_file() {
            continue;
        }
        
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();
        
        print!("\x1b[38;2;130;130;130mReplacing {}\x1b[0m", filename_str);
        io::stdout().flush().ok();
        
        let source = temp_dir.join(&filename);
        let dest = exe_dir.join(&filename);
        
        match fs::copy(&source, &dest) {
            Ok(_) => {
                println!("\r\x1b[K\x1b[38;2;130;130;130mReplacing {}\x1b[0m \x1b[38;2;50;200;50mDone\x1b[0m", filename_str);
            }
            Err(e) => {
                println!("\r\x1b[K\x1b[38;2;130;130;130mReplacing {}\x1b[0m \x1b[38;2;255;50;50mFailed: {}\x1b[0m", filename_str, e);
                all_ok = false;
                break;
            }
        }
    }
    
    println!();
    
    if all_ok {
        fs::remove_dir_all(&temp_dir).ok();
        println!("\x1b[38;2;50;200;50mUpdate complete!\x1b[0m\n");
        thread::sleep(Duration::from_secs(2));
    } else {
        println!("\x1b[38;2;255;50;50mUpdate failed!\x1b[0m");
        println!("\x1b[38;2;255;255;255mBackup saved in 'update_backup'.\x1b[0m");
        println!("\x1b[38;2;255;255;255mUse 'todo rollback' to restore.\x1b[0m\n");
        wait_for_key();
    }
}

fn rollback_update() {
    println!("\nRolling back...\n");
    
    thread::sleep(Duration::from_millis(300));
    
    let exe_dir = match get_exe_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error_and_wait(&format!("Cannot find exe directory: {}", e));
            return;
        }
    };
    
    let backup_dir = exe_dir.join("update_backup");
    
    if !backup_dir.exists() {
        error_and_wait("update_backup not found");
        return;
    }
    
    let entries = match fs::read_dir(&backup_dir) {
        Ok(entries) => entries,
        Err(e) => {
            error_and_wait(&format!("Cannot read update_backup: {}", e));
            return;
        }
    };
    
    let mut all_ok = true;
    
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        if !entry.path().is_file() {
            continue;
        }
        
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();
        
        #[cfg(windows)]
        let updater_name = "updater.exe";
        #[cfg(not(windows))]
        let updater_name = "updater";
        
        if filename_str == updater_name {
            continue;
        }
        
        print!("\x1b[38;2;130;130;130mRestoring {}\x1b[0m", filename_str);
        io::stdout().flush().ok();
        
        let source = backup_dir.join(&filename);
        let dest = exe_dir.join(&filename);
        
        match fs::copy(&source, &dest) {
            Ok(_) => {
                println!("\r\x1b[K\x1b[38;2;130;130;130mRestoring {}\x1b[0m \x1b[38;2;50;200;50mDone\x1b[0m", filename_str);
            }
            Err(e) => {
                println!("\r\x1b[K\x1b[38;2;130;130;130mRestoring {}\x1b[0m \x1b[38;2;255;50;50mFailed: {}\x1b[0m", filename_str, e);
                all_ok = false;
                break;
            }
        }
    }
    
    println!();
    
    if all_ok {
        let temp_dir = exe_dir.join("update_temp");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).ok();
        }
        
        println!("\x1b[38;2;50;200;50mRollback complete!\x1b[0m\n");
        thread::sleep(Duration::from_secs(2));
    } else {
        println!("\x1b[38;2;255;50;50mRollback failed!\x1b[0m\n");
        wait_for_key();
    }
}

fn get_exe_dir() -> io::Result<PathBuf> {
    let exe_path = env::current_exe()?;
    exe_path.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Cannot find parent directory"))
        .map(|p| p.to_path_buf())
}

fn wait_for_key() {
    print!("\x1b[38;2;130;130;130mPress any key to continue...\x1b[0m");
    io::stdout().flush().ok();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
}

fn error_and_wait(msg: &str) {
    println!("\x1b[38;2;255;50;50mError:\x1b[0m \x1b[38;2;255;255;255m{}\x1b[0m\n", msg);
    wait_for_key();
}