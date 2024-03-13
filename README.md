# Project Bootstrap

Adds a .gitignore, LICENSE and README file to your new project.

## Installation

You might want to edit the author name in the NOTICE text before you compile the code.

    echo "GITHUB_TOKEN=<YOUR AUTH TOKEN>" > .env
    cargo install --path .

## Usage

    project-bootstrap <language> <license> --name <project-name>

`license` and `project-name` are both optional. `license` defaults to Apache-2.0, and `project-name`
to the name of your directory. 


##### This project is maintained by [Abhinav Chennubhotla](https://github.com/PhoenixFlame101).
