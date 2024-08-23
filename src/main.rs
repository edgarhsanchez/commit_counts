//! This program counts the number of commits by each user across all Git repositories in a directory.
//!
//! It also prints the remote origins of each repository.
//! Use it like this:
//! ./commit_counter /path/to/directory

use git2::{Error, Repository};
use rayon::prelude::*;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Error> {
    // Check for a command-line argument, otherwise default to the current directory
    let args: Vec<String> = env::args().collect();
    let start_path = if args.len() > 1 { &args[1] } else { "." };

    let git_dirs = find_git_dirs(start_path);

    // Arc and Mutex to safely accumulate commit counts and project origins across threads
    let total_commits_by_user = Arc::new(Mutex::new(HashMap::new()));
    let project_origins = Arc::new(Mutex::new(Vec::new()));

    // Process each Git repository in parallel using Rayon
    git_dirs.par_iter().for_each(|repo_path| {
        let repo_path = repo_path.clone();
        let total_commits_by_user = Arc::clone(&total_commits_by_user);
        let project_origins = Arc::clone(&project_origins);

        if let Ok(repo) = Repository::open(repo_path) {
            if let Ok(origin_url) = get_remote_origin_url(&repo) {
                let mut origins = project_origins.lock().unwrap();
                origins.push(origin_url);
            }

            if let Ok(commit_counts) = count_commits_by_user(&repo) {
                let mut total_commits = total_commits_by_user.lock().unwrap();
                for (user, count) in commit_counts {
                    *total_commits.entry(user).or_insert(0) += count;
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

    Ok(())
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

fn get_remote_origin_url(repo: &Repository) -> Result<String, Error> {
    if let Ok(remote) = repo.find_remote("origin") {
        if let Some(url) = remote.url() {
            return Ok(url.to_string());
        }
    }
    Err(Error::from_str("No remote origin found"))
}

fn count_commits_by_user(repo: &Repository) -> Result<HashMap<String, i32>, Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut commits_by_user = HashMap::new();

    for commit_id in revwalk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?.clone();
        let author = commit.author();
        if let Some(name) = author.name() {
            let email = author.email().unwrap_or("Unknown Email");
            let count = commits_by_user.entry(format!("{}", email)).or_insert(0);
            *count += 1;
        }
    }

    Ok(commits_by_user)
}
