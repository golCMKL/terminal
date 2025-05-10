use std::env;
use std::path::Path;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, execvp, pipe, dup2, ForkResult};
use std::fs::File;
use std::process::exit;
use std::io::{self, Write};
use std::os::unix::io::{AsRawFd};
use std::os::fd::OwnedFd;
use std::ffi::CString;


fn main() {

	loop{

		match env::current_dir() 
        {

			Ok(path) => print!("{}$ ", path.display()),
			Err(e) => 
            {
				eprintln!("Error getting directory: {}",e);
				continue;
			}
		}
		io::stdout().flush().unwrap();

	
        
		let mut line = String::new();
		if std::io::stdin().read_line(&mut line).is_err() 
        {

			eprintln!("Failed to read output");
			continue;
		}

		let input = line.trim();
		if input.is_empty() {
			continue;
		}

		let mut parts = input.split_whitespace();
		let command = parts.next().unwrap();
		let args: Vec<&str> = parts.collect();

		match command {
			"exit" => exit(0),
			"cd" => 
            {
				if args.is_empty() 
                {
					eprintln!("Missing operand");
				} 
                else 
                {
					let new_dir = Path::new(args[0]);
					if let Err(e) = env::set_current_dir(new_dir) 
                    {
						eprintln!("cd: {} {}", args[0], e);
					}
				} 
				continue;
			}
			_ => {}
		}

		let background = input.ends_with('&');
        let input = if background { &input[..input.len() - 1] } else { input };
        
        
        let mut input_file: Option<File> = None; //set values
        let mut output_file: Option<File> = None;
        let mut commands: Vec<Vec<&str>> = vec![]; //array of array
        let mut temp_command: Vec<&str> = vec![];

        let mut iterator = input.split_whitespace().peekable();
        let mut error_flag = false;

        while let Some(token) = iterator.next()
        {
            match token 
            {
                "<" => 
                {
                    if let Some(file) = iterator.next() 
                    {
                        match File::open(file) 
                        {
                            Ok(f) => input_file = Some(f),
                            Err(e) => 
                            {
                                eprintln!("Error opening file {}: {}", file, e);
                                error_flag = true;
                                break;
                                //continue;
                            }
                        }
                    }
                }
                ">" => 
                {
                    if let Some(file) = iterator.next() 
                    {
                        match File::create(file) 
                        {
                            Ok(f) => output_file = Some(f),
                            Err(e) => 
                            {
                                eprintln!("Error creating file {}: {}", file, e);
                                error_flag = true;
                                break;
                                //continue;
                            }
                        }
                    }
                }
                "|" => 
                {
                    commands.push(temp_command.clone());
                    temp_command.clear();
                }
                _ => temp_command.push(token),
            }
        }

        if error_flag { 
            continue; 
        }

        if !temp_command.is_empty() 
        {
            commands.push(temp_command);
        }

        let mut previous_fd: Option<OwnedFd> = None;

        for (i, cmd_parts) in commands.iter().enumerate() {
            let c_command = CString::new(cmd_parts[0]).unwrap();
            let c_args: Vec<CString> = cmd_parts.iter().map(|&arg| CString::new(arg).unwrap()).collect();

            let (pipe_read, pipe_write) = if i < commands.len() - 1 
            {
                let (r, w) = pipe().unwrap();
                (Some(r), Some(w))
            } 
            else 
            {
                (None, None)
            };

            match unsafe { fork() } {
                Ok(ForkResult::Child) => {
                    if let Some(ref fd) = previous_fd {
                        dup2(fd.as_raw_fd(), 0).unwrap();
                        let _ = fd;
                    }
                    if let Some(fd) = pipe_write {
                        dup2(fd.as_raw_fd(), 1).unwrap();
                        drop(fd);
                    }
                    if let Some(ref file) = input_file {
                        dup2(file.as_raw_fd(), 0).unwrap();
                    }
                    if let Some(ref file) = output_file {
                        dup2(file.as_raw_fd(), 1).unwrap();
                    }
                    if let Err(e) = execvp(&c_command, &c_args) {
                        eprintln!("Error executing {}: {}", cmd_parts[0], e);
                        exit(1);
                    }
                }
                Ok(ForkResult::Parent { child }) => {
                    if let Some(fd) = previous_fd {
                        drop(fd);
                    }
                    previous_fd = pipe_read;
                    if let Some(fd) = pipe_write {
                        drop(fd);
                    }
            
                    
                    if !background {
                        match waitpid(child, None) 
                        {
                            Ok(WaitStatus::Exited(_, status)) => println!("Process exited with status {}", status),
                            Ok(_) => println!("Process ended"),
                            Err(e) => eprintln!("Error waiting for process: {}", e),
                        }
                    } else {
                        println!("Started process in background (PID: {})", child);
                    }
                }
                Err(e) => {
                    eprintln!("Fork failed: {}", e);
                }
            
            }
        }
	}
}