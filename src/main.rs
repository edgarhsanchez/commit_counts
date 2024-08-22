//! This program counts the number of commits by each user across all Git repositories in a directory.
//! 
//! It also prints the remote origins of each repository.
//! Use it like this:
//! ./commit_counter /path/to/directory


use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use git2::Repository;
use rayon::prelude::*;
use std::sync::Mutex;


fn main() {
    // Check for a command-line argument, otherwise default to current directory
    let args: Vec<String> = env::args().collect();
    let start_path = if args.len() > 1 {
        &args[1]
    } else {
        "."
    };

    let git_dirs = find_git_dirs(start_path);

    // Mutex to accumulate commit counts and project origins safely across threads
    let total_commits_by_user = Mutex::new(HashMap::new());
    let project_origins = Mutex::new(Vec::new());

    // Process each Git directory in parallel
    git_dirs.par_iter().for_each(|git_dir| {
        if let Ok(repo) = Repository::open(git_dir) {
            match count_commits_by_user(&repo) {
                Ok(commit_counts) => {
                    let mut total_commits = total_commits_by_user.lock().unwrap();
                    for (user, count) in commit_counts {
                        *total_commits.entry(user).or_insert(0) += count;
                    }

                    if let Some(origin_url) = get_remote_origin_url(&repo) {
                        let mut origins = project_origins.lock().unwrap();
                        origins.push(origin_url);
                    }
                }
                Err(err) => {
                    eprintln!("Failed to count commits in repository {}: {}", git_dir, err);
                }
            }
        }
    });

    // Get the final commit counts and sort them
    let total_commits_by_user = total_commits_by_user.lock().unwrap().clone();
    let mut commit_counts: Vec<_> = total_commits_by_user.iter().collect();
    commit_counts.sort_by(|a, b| b.1.cmp(a.1));

    let project_origins = project_origins.lock().unwrap();

    // Display the results
    println!("Commit counts by user across all repositories (sorted):");
    for (user, count) in commit_counts {
        println!("{}: {}", user, count);
    }

    println!("\nRepositories with their remote origins:");
    for origin in project_origins.iter() {
        println!("{}", origin);
    }
}

fn find_git_dirs(start_path: &str) -> Vec<String> {
    let mut git_dirs = Vec::new();
    let start_path = Path::new(start_path);
    if start_path.is_dir() {
        for entry in fs::read_dir(start_path).expect("Directory not found") {
            let entry = entry.expect("Unable to read entry");
            let path = entry.path();
            if path.is_dir() {
                if path.join(".git").exists() {
                    git_dirs.push(path.to_string_lossy().to_string());
                } else {
                    let mut sub_git_dirs = find_git_dirs(&path.to_string_lossy());
                    git_dirs.append(&mut sub_git_dirs);
                }
            }
        }
    }
    git_dirs
}

fn count_commits_by_user(repo: &Repository) -> Result<HashMap<String, i32>, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut commits_by_user = HashMap::new();

    revwalk.map(|commit| {
        if let Ok(commit) = commit {
            if let Ok(commit) = repo.find_commit(commit) {
                let (first, last) = get_first_and_last(commit.author().name().unwrap_or("Unknown").to_string());
                // Concatenate first and last name
                let name = format!("{} {}", first, last);
                let count = commits_by_user.entry(name).or_insert(0);
                *count += 1;
            }
        }
    }).for_each(drop);

    Ok(commits_by_user)
}

// get the first and last name of the author
fn get_first_and_last(author: String) -> (String, String) {
    // if email return email
    if author.contains('@') {
        return (author, "".to_string());
    }

    let binding = author.to_ascii_lowercase();
    let mut parts = binding.split_whitespace();
    let first = parts.next().unwrap_or_default().trim();
    let last = parts.last().unwrap_or_default().trim();

    // if the first name contains a comma, it's likely in the format "Last, First"
    if first.contains(',') {
        let mut parts = first.split(',');
        let last = parts.next().unwrap_or_default().trim();
        let first = parts.next().unwrap_or_default().trim();
        return (first.to_string(), last.to_string());
    }

    (first.to_string(), last.to_string())
}



fn get_remote_origin_url(repo: &Repository) -> Option<String> {
    if let Ok(remote) = repo.find_remote("origin") {
        if let Some(url) = remote.url() {
            return Some(url.to_string());
        }
    }
    None
}