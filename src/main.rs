use dotenv::dotenv;

use curl::easy::{Easy, List};
use serde_json;

use std::env;
use std::fs;
use std::io::Write;
use std::str::FromStr;

use chrono::Datelike;
use convert_case::{Case, Casing};

use clap::{Arg, Command};
use dialoguer::{theme::ColorfulTheme, Select};

fn download_file(html_url: &str, filename: &str) {
    let raw_url = html_url.replace("/blob/", "/raw/");

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(filename)
        .expect("error opening file");

    let mut easy = Easy::new();
    easy.url(&raw_url).unwrap();
    let _ = easy.follow_location(true);
    let mut file_data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                file_data.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let _ = file.write(&file_data);
}

fn pick_and_download_gitignore(language: &str, github_token: &str) {
    // init easy handle and url
    let mut easy = Easy::new();
    easy.url(
        &format!("https://api.github.com/search/code?q=repo%3Agithub%2Fgitignore+{language}")[..],
    )
    .unwrap();

    // init required headers for github api interfacing
    let mut headers = List::new();
    headers
        .append("Accept: application/vnd.github+json")
        .expect("accept header incorrect");
    headers
        .append(&format!("Authorization: Bearer {github_token}")[..])
        .expect("auth token incorrect");
    headers
        .append("User-Agent: gitignore-add")
        .expect("user agent incorrect");
    easy.http_headers(headers).expect("wrong headers");

    // write http response to buffer, and convert to json
    let mut search_response = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                search_response.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let search_response = String::from_utf8(search_response).unwrap();
    let search_response = serde_json::Value::from_str(&search_response).unwrap();

    // download .gitignore directly or show a prompt incase of multiple matches
    if search_response["total_count"] == 1 {
        let html_url = search_response["items"][0]["html_url"].as_str().unwrap();
        download_file(&html_url, ".gitignore");
    } else {
        let mut options = Vec::new();
        let mut urls = Vec::new();

        for item in search_response["items"].as_array().unwrap() {
            options.push(item["name"].as_str().unwrap());
            urls.push(item["html_url"].as_str().unwrap());
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Which .gitignore do you want to use?")
            .items(&options)
            .interact()
            .unwrap();

        download_file(urls[selection], ".gitignore");
    }
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

    fs::write("LICENSE", license_body).expect("Can't write LICENSE file");
    if license_name == "apache-2.0" {
        make_apache_notice(project_name, author)
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

    fs::write("README.md", readme).expect("Can't write README.md");
}

fn main() {
    // env variables
    dotenv().ok();
    let github_token = std::env::var("GITHUB_TOKEN").expect("GitHub Token must be set");

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

    pick_and_download_gitignore(&language, &github_token);
    pick_and_download_license(&license, &github_token, &project_name, author);
    make_readme(&project_name, author, author_github)
}
