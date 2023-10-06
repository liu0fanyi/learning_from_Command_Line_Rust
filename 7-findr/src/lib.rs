use crate::EntryType::*;
use clap::{App, Arg};
use regex::Regex;
use std::error::Error;
use walkdir::{WalkDir, DirEntry};

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Eq, PartialEq)]
enum EntryType {
    Dir,
    File,
    Link,
}

#[derive(Debug)]
pub struct Config {
    paths: Vec<String>,
    names: Vec<Regex>,
    entry_types: Vec<EntryType>,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("findr")
        .version("0.1.0")
        .author("Ken Youens-Clark <kyclark@gmail.com>")
        .about("Rust find")
        .arg(
            Arg::with_name("paths")
                .value_name("PATH")
                .help("Search paths")
                .default_value(".")
                .multiple(true),
        )
        .arg(
            Arg::with_name("names")
                .value_name("NAME")
                .short("n")
                .long("name")
                .help("Name")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("types")
                .value_name("TYPE")
                .short("t")
                .long("type")
                .help("Entry type")
                .possible_values(&["f", "d", "l"])
                .takes_value(true)
                .multiple(true),
        )
        .get_matches();

    let names = matches
        .values_of_lossy("names")
        // 这个是Option::map返回的还是Option
        .map(|vals| {
            vals.into_iter()
                .map(|name| {
                    // 这个返回一个Result
                    Regex::new(&name)
                        // 对于invalid regexes，使用Result::map_err来创建一个可读的错误信息
                        .map_err(|_| format!("Invalid --name \"{}\"", name))
                })
                // 收集成Result<Vec>
                .collect::<Result<Vec<_>, _>>()
        })
        // 从Option::map返回，会多个Option
        // 将Option of a Result转换成Result of a Option
        .transpose()?
        // 解包失败用默认，默认是个empty vector
        .unwrap_or_default();

    let entry_types = matches
        .values_of_lossy("types")
        // 使用Option::map handle Some(vals)
        .map(|vals| {
            vals.iter()
                // Iterator::map to check each of the provided values
                .map(|val| match val.as_str() {
                    "d" => Dir,
                    "f" => File,
                    "l" => Link,
                    _ => unreachable!("Invalid type"),
                })
                // auto infers that I want a Vec<EntryType>
                .collect()
        })
        // default empty vector
        .unwrap_or_default();

    Ok(Config{
        paths: matches.values_of_lossy("paths").unwrap(),
        names,
        entry_types,
    })
}

pub fn run(config: Config) -> MyResult<()> {
    // Create a similar closure to filter entries by any of the types.
    let type_filter = |entry: &DirEntry| {
        config.entry_types.is_empty()
            || config
                .entry_types
                .iter()
                // 只要iter()里有一个true，整体是true
                .any(|entry_type| match entry_type {
                    Link => entry.file_type().is_symlink(),
                    Dir => entry.file_type().is_dir(),
                    File => entry.file_type().is_file(),
                    })

    };

    // Create a closure to filter entries on any of the regular expressions.
    let name_filter = |entry: &DirEntry| {
        config.names.is_empty()
            || config
                .names
                .iter()
                .any(|re| re.is_match(&entry.file_name().to_string_lossy()))
    };

    for path in config.paths {
        // each directory entry is returned as a Result所有dir返回一个Result
        let entries = WalkDir::new(path)
            // 返回的是一个Result<DirEntry>
            .into_iter()
            // 错误的输入STDERR，返回NONE，正确的通过进入下一个filter
            .filter_map(|e| match e {
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
                Ok(entry) => Some(entry),
            })
            .filter(type_filter)
            .filter(name_filter)
            .map(|entry| entry.path().display().to_string())
            .collect::<Vec<_>>();
        println!("{}", entries.join("\n"));
    }

    Ok(())
}

