use dotenv::dotenv;

use curl::easy::{Easy, List};
use serde_json;
use tempfile::Builder;
use walkdir::WalkDir;

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command as ProcessCommand;
use std::str::FromStr;

use chrono::Datelike;
use convert_case::{Case, Casing};

use clap::{Arg, Command};
use dialoguer::{theme::ColorfulTheme, Select};

fn pick_and_download_gitignore(language: &str) {
    let tmp_dir = Builder::new()
        .prefix("gitignore")
        .tempdir()
        .expect("failed to create temp dir");
    let repo_url = "https://github.com/github/gitignore.git";

    let status = ProcessCommand::new("git")
        .arg("clone")
        .arg(repo_url)
        .arg(tmp_dir.path())
        .status()
        .expect("failed to clone repo");

    if !status.success() {
        panic!("failed to clone repo");
    }

    let mut matching_files = Vec::new();
    for entry in WalkDir::new(tmp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem() {
                if let Some(stem_str) = stem.to_str() {
                    if stem_str.to_lowercase().contains(&language.to_lowercase()) {
                        matching_files.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    let mut file_path = None;
    let language_lower = language.to_lowercase();

    let exact_match_index = matching_files.iter().position(|path| {
        path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_lowercase()
            == language_lower
    });

    if matching_files.len() == 1 && exact_match_index.is_some() {
        file_path = Some(matching_files[0].clone());
    } else if !matching_files.is_empty() {
        let options: Vec<String> = matching_files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Which .gitignore do you want to use?")
            .items(&options)
            .interact()
            .unwrap();

        file_path = Some(matching_files[selection].clone());
    } else {
        println!("No matching .gitignore found for {}", language);
    }

    if let Some(path) = file_path {
        let mut gitignore_mode = "overwrite";

        if fs::metadata(".gitignore").is_ok() {
            let options = &["Overwrite", "Append", "Cancel"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(".gitignore already exists. What do you want to do?")
                .items(options)
                .default(0)
                .interact()
                .unwrap();

            match selection {
                0 => gitignore_mode = "overwrite",
                1 => gitignore_mode = "append",
                2 => {
                    println!("Operation cancelled.");
                    tmp_dir.close().expect("failed to close temp dir");
                    return;
                }
                _ => unreachable!(),
            }
        }

        let mut open_options = fs::OpenOptions::new();
        if gitignore_mode == "append" {
            open_options.append(true).create(true);
        } else {
            open_options.write(true).create(true).truncate(true);
        }

        let mut file = open_options
            .open(".gitignore")
            .expect("failed to open .gitignore");

        let content = fs::read_to_string(path).expect("failed to read gitignore content");
        file.write_all(content.as_bytes())
            .expect("failed to write to .gitignore");
    }

    tmp_dir.close().expect("failed to close temp dir");
}

fn pick_and_download_license(license: &str, github_token: &str, project_name: &str, author: &str) {
    // init easy handle and url
    let mut easy = Easy::new();
    easy.url(&"https://api.github.com/licenses"[..]).unwrap();

    // init required headers for github api interfacing
    let mut headers = List::new();
    headers
        .append("Accept: application/vnd.github+json")
        .expect("accept header incorrect");
    headers
        .append(&format!("Authorization: Bearer {github_token}")[..])
        .expect("auth token incorrect");
    headers
        .append("User-Agent: project-bootstrap")
        .expect("user agent incorrect");
    easy.http_headers(headers).expect("wrong headers");

    // write http response to buffer, and convert to json
    let mut licenses = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                licenses.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let licenses = String::from_utf8(licenses).unwrap();
    let licenses = serde_json::Value::from_str(&licenses).unwrap();

    // search for license in the response keys
    let mut matching_licenses: Vec<(String, String)> = Vec::new();
    for item in licenses.as_array().unwrap() {
        if item["key"].as_str().unwrap().contains(license) {
            matching_licenses.push((
                item["key"].as_str().unwrap().to_string(),
                item["spdx_id"].as_str().unwrap().to_string(),
            ));
        }
    }

    let mut license_name = String::from("apache");

    if matching_licenses.len() == 1 {
        license_name = format!("{}", matching_licenses[0].0);
        let url = &format!("https://api.github.com/licenses/{}", license_name);
        easy.url(&url[..]).unwrap();
    } else {
        let mut options = Vec::new();
        let mut urls = Vec::new();

        for item in matching_licenses {
            options.push(item.1);

            license_name = String::from(item.0);
            urls.push(format!("https://api.github.com/licenses/{}", license_name));
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Which license do you want to use?")
            .items(&options)
            .interact()
            .unwrap();

        easy.url(&urls[selection][..]).unwrap();
    }

    // write http response to buffer, and convert to json
    let mut license = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                license.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let license = String::from_utf8(license).unwrap();
    let license = serde_json::Value::from_str(&license).unwrap();
    let license_body = license["body"].as_str().unwrap();

    // Write LICENSE if it doesn't already exist
    if fs::metadata("LICENSE").is_err() {
        fs::write("LICENSE", license_body).expect("Can't write LICENSE file");
        if license_name == "apache-2.0" {
            make_apache_notice(project_name, author)
        }
    }
}

fn make_apache_notice(project_name: &str, author: &str) {
    let notice = format!(
        "{project_name}
Copyright {} {author}

{project_name} was originally developed and maintained by {author}.

Portions of this software were developed by various contributors, who retain
copyright on their work. These works are licensed to {author}.

{project_name} is available under the Apache 2.0 license. See LICENSE for more
information.

This software uses other open source libraries. These libraries have their own
licenses and copyright holders.",
        chrono::Utc::now().year()
    );

    fs::write("NOTICE", notice).expect("Can't write NOTICE file");
}

fn make_readme(project_name: &str, author: &str, author_github: &str) {
    let readme = format!(
        "# {project_name}

##### This project is maintained by [{author}]({author_github}).
"
    );

    // Write README if it doesn't already exist
    if fs::metadata("README.md").is_err() {
        fs::write("README.md", readme).expect("Can't write README.md");
    }
}

fn main() {
    // env variables
    dotenv().ok();

    // retrieve cli arguments
    let matches = Command::new("Project Bootstrap")
        .author("Abhinav Chennubhotla <abhinav@chennubhotla.com>")
        .about("Adds a .gitignore, LICENSE and README file to your new project.")
        .arg(
            Arg::new("language")
                .help("Sets the programming language for the .gitignore")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("license")
                .help("Sets the open-source license")
                .required(false)
                .default_value("apache")
                .index(2),
        )
        .arg(
            Arg::new("name")
                .long("name")
                .value_name("")
                .required(false)
                .help("Sets a custom project name"),
        )
        .get_matches();

    let cwd = env::current_dir().unwrap();
    let cwd = cwd.file_name().unwrap().to_str().unwrap();
    let cwd = String::from(cwd);

    let language = matches.get_one::<String>("language").unwrap();
    let license = matches.get_one::<String>("license").unwrap();
    let author = "Abhinav Chennubhotla";
    let author_github = "https://github.com/PhoenixFlame101/";

    let project_name = matches.get_one::<String>("name").unwrap_or(&cwd);
    let project_name = project_name
        .split(|c| c == '-' || c == '_')
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .join(" ")
        .split_whitespace()
        .map(|s| s.to_case(Case::Title))
        .collect::<Vec<String>>()
        .join(" ");

    pick_and_download_gitignore(&language);
    let github_token = std::env::var("GITHUB_TOKEN").expect("GitHub Token must be set");
    pick_and_download_license(&license, &github_token, &project_name, author);
    make_readme(&project_name, author, author_github)
}
