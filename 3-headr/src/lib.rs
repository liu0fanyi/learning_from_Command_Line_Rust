use clap::{App, Arg};
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Config {
    files: Vec<String>,
    lines: usize,
    bytes: Option<usize>,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("headr")
        .version("0.1.0")
        .author("Ken Youens-Clark <kylark@gmail.com>")
        .about("Rust head")
        .arg(
            Arg::with_name("lines")
            .short("n")
            .long("lines")
            .value_name("LINES")
            .help("Number of lines")
            .default_value("10")
        )
        .arg(
            Arg::with_name("bytes")
            .short("c")
            .long("bytes")
            .value_name("BYTES")
            // 可能没有设置defaultvalue，所以得设置takes_value
            .takes_value(true)
            .conflicts_with("lines")
            .help("Number of bytes")
        )
        .arg(
            Arg::with_name("files")
            .value_name("FILE")
            .help("Input file(s)")
            // 允许多个输入
            .multiple(true)
            .default_value("-")
        )
        .get_matches();

    // 这里是正整数检查
    let lines = matches
        // 返回的是Option<&str>
        .value_of("lines")
        // map能拆开Option然后传给parse_positive_int
        .map(parse_positive_int)
        // 上面返回是Option<Result>
        // transpose能把这个转换成Result<Option>
        .transpose()
        // 提供一个informative error message, 返回的应该也是个result吧，不太清楚用法
        .map_err(|e| format!("illegal line count -- {}", e))?;

    let bytes = matches
        .value_of("bytes")
        .map(parse_positive_int)
        .transpose()
        .map_err(|e| format!("illegal byte count -- {}", e))?;

    Ok(Config{
        files: matches.values_of_lossy("files").unwrap(),
        lines: lines.unwrap(),
        bytes,
    })
}

pub fn run(config: Config) -> MyResult<()> {
    // 或者使用dbg!
    // Ok(println!("{:#?}", config))
    let num_files = config.files.len();
    for (file_num, filename ) in config.files.iter().enumerate() {
        match open(&filename) {
            Err(err) => eprintln!("Failed to open{}: {}", filename, err),
            Ok(mut file) => {
                if num_files > 1 {
                    println!(
                        "{}==> {} <==",
                        if file_num > 0 { "\n" } else { "" },
                        filename
                    );
                }
                if let Some(num_bytes) = config.bytes {
                    let mut handle = file.take(num_bytes as u64);
                    let mut buffer = vec![0; num_bytes];
                    let bytes_read = handle.read(&mut buffer)?;
                    print!("{}", String::from_utf8_lossy(&buffer[..bytes_read]));
                } else {
                    let mut line = String::new();
                    for _ in 0..config.lines {
                        let bytes = file.read_line(&mut line)?;
                        if bytes == 0 {
                            break;
                        }
                        print!("{}", line);
                        line.clear();
                    }
                }
            }
        }
    }
    Ok(())
}

// --------------------------------------------------
fn open(filename: &str) -> MyResult<Box<dyn BufRead>> {
    match filename {
        "-" => Ok(Box::new(BufReader::new(io::stdin()))),
        _ => Ok(Box::new(BufReader::new(File::open(filename)?)))
    }
}
// --------------------------------------------------
fn parse_positive_int(val: &str) -> MyResult<usize> {
    // 这个东西会给一个panic
    // unimplemented!();
    // 这里会根据返回值就推断出是usize, 够远
    match val.parse() {
        // 这里有个守卫guard if n > 0
        Ok(n) if n > 0 => Ok(n),
        // 直接给val，人要一个impl Error的Box所以会报错
        _ => Err(From::from(val)),
    }
}

// 仅针对某一个function的unit test
#[test]
fn test_parse_positive_int() {
    // 3 is an OK integer
    let res = parse_positive_int("3");
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 3);

    // Any string is an error
    let res = parse_positive_int("foo");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().to_string(), "foo".to_string());

    // A zero is an error
    let res = parse_positive_int("0");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().to_string(), "0".to_string());
}

