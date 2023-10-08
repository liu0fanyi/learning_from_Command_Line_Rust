use crate::Extract::*;
use clap::{App, Arg};
use regex::Regex;
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader},
    num::NonZeroUsize,
    ops::Range,
};

type MyResult<T> = Result<T, Box<dyn Error>>;
type PositionList = Vec<Range<usize>>;

#[derive(Debug)]
pub enum Extract {
    Fields(PositionList),
    Bytes(PositionList),
    Chars(PositionList),
}

#[derive(Debug)]
pub struct Config {
    files: Vec<String>,
    delimiter: u8,
    // 暂时不确定是啥
    extract: Extract,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("cutr")
            .version("0.1.0")
            .author("Ken Youens-Clark <kyclark@gmail.com>")
            .about("Rust cut")
            .arg(
                Arg::with_name("files")
                    .value_name("FILE")
                    .help("Input file(s)")
                    .multiple(true)
                    .default_value("-"),
            )
            .arg(
                Arg::with_name("delimiter")
                    .value_name("DELIMITER")
                    .short("d")
                    .long("delim")
                    .help("Field delimiter")
                    // 默认是tab
                    .default_value("\t"),
            )
            .arg(
                Arg::with_name("fields")
                    .value_name("FIELDS")
                    .short("f")
                    .long("fields")
                    .help("Selected fields")
                    // 与chars和bytes冲突
                    .conflicts_with_all(&["chars", "bytes"]),
            )
            .arg(
                Arg::with_name("bytes")
                    .value_name("BYTES")
                    .short("b")
                    .long("bytes")
                    .help("Selected bytes")
                    .conflicts_with_all(&["fields", "chars"]),
            )
            .arg(
                Arg::with_name("chars")
                    .value_name("CHARS")
                    .short("c")
                    .long("chars")
                    .help("Selected characters")
                    .conflicts_with_all(&["fields", "bytes"]),
            )
            .get_matches();

    let delimiter = matches.value_of("delimiter").unwrap();
    let delim_bytes = delimiter.as_bytes();
    if delim_bytes.len() != 1 {
        return Err(From::from(format!(
            "--delim \"{}\" must be a single byte",
            delimiter
        )))
    }

    let fields = matches.value_of("fields").map(parse_pos).transpose()?;
    let bytes = matches.value_of("bytes").map(parse_pos).transpose()?;
    let chars = matches.value_of("chars").map(parse_pos).transpose()?;

    let extract = if let Some(field_pos) = fields {
        Fields(field_pos)
    } else if let Some(byte_pos) = bytes {
        Bytes(byte_pos)
    } else if let Some(char_pos) = chars {
        Chars(char_pos)
    } else {
        return Err(From::from("Must have --fields, --bytes, or --chars"));
    };

    Ok(Config {
        files: matches.values_of_lossy("files").unwrap(),
        // 已经检查过只有一个，unwrap么有问题
        // 这个delim_bytes是个&u8, 需要解引用才能copy给delimiter
        delimiter: *delim_bytes.first().unwrap(),
        extract,
    })
}

fn parse_index(input: &str) -> Result<usize, String> {
    // 搞个closure
    let value_error = || format!("illegal list value: \"{}\"", input);
    input
        // starts_with, str功能，如果从+开始
        .starts_with("+")
        // 那么就报错
        .then(|| Err(value_error()))
        // 解包或
        .unwrap_or_else(|| {
            // 不然
            input
                // parse成非0 usize, positive integer value
                .parse::<NonZeroUsize>()
                // 当input vlaue parses 成功，cast the value to a usize, 然后-1
                // 为了对齐0 index, 用户输入1其实是第0位
                .map(|n| usize::from(n) - 1)
                // 如果失败，输出错误信息
                .map_err(|_| value_error())
        })
}

fn parse_pos(range: &str) -> MyResult<PositionList> {
    // 开头结尾匹配，俩integer，- dash 连接
    // 加了r, rust默认不做转义，不然会问\d是啥东西
    // ()内的东西可以用captures捕获，是1 based counting,所以第一个在1，第二个在2
    let range_re = Regex::new(r"^(\d+)-(\d+)$").unwrap();
    range
        // 用comma分开
        .split(',')
        .into_iter()
        .map(|val| {
            // 如果parse_index成功解析出来single的interger, 就是固定某一column的cut
            // 则cut n..n+1列的内容(其实就是处理n自己)
            parse_index(val).map(|n| n..n+1).or_else(|e| {
                // 不然用正则匹配, 失败就e
                // 成功就继续提取captures的1和2, 在括号parenttheses里的可以被capture，捕获
                range_re.captures(val).ok_or(e).and_then(|captures| {
                    let n1 = parse_index(&captures[1])?;
                    let n2 = parse_index(&captures[2])?;
                    // 第一个不能>=第二个
                    if n1 >= n2 {
                        return Err(format!(
                            "First number in range ({}) \
                            must be lower than second number ({})",
                            n1 + 1,
                            n2 + 1
                        ));
                    }
                    // 不然返回成功的值
                    Ok(n1..n2 + 1)
                })
            })
        })
        // 拼成Result
        .collect::<Result<_, _>>()
        // 错误信息都用From::from做出来，前面有提，这个东西可以转任意错误到给定错误
        .map_err(From::from)
}

// 一开始作者在这里使用了&PositionList作为char_pos的入参类型
// rust 建议改成&[Range<usize>], 这能少一点restrictive
fn extract_chars(line: &str, char_pos: &[Range<usize>]) -> String {
    // 用chars得到所有的characters, Vec不能省略因为collect那省略了, collect可以搞成各种类型
    let chars: Vec<_> = line.chars().collect();
    /*// 用来hold selected characters
    let mut selected: Vec<char> = vec![];

    /*// iter char_pos, 
    for range in char_pos.iter().cloned() {
        // iter range
        for i in range {
            // 如果get(i)成功(可能出错，比如range范围超出了chars的长度)，则push
            if let Some(Val) = chars.get(i) {
                selected.push(*val)
            }
        }
    }*/
    // 更简单的方法，直接selected扩展，range filter_map的内容
    // filter_map跟filter的区别，filter返回的是true内容, filter_map返回的是Some(x)的内容
    // 一个针对bool一个针对Result
    for range in char_pos.iter().cloned() {
        selected.extend(range.filter_map(|i| chars.get(i)));
    }

    // 最后收集
    selected.iter().collect();
    */
    // 以及一个更简单的方法
    /*char_pos
        .iter()
        .cloned()
        .map(|range| range.filter_map(|i| chars.get(i)))
        // flattern remove nested structures
        // 去掉一层iter，这里就把返回的FilterMap<std::ops::Range<usize>>给干掉了一层
        // 说实话不太懂 TODO:
        .flatten()
        .collect()*/
    // 终极，flatten + map
    char_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| chars.get(i)))
        .collect()
}

fn extract_bytes(line: &str, byte_pos: &[Range<usize>]) -> String {
    let bytes = line.as_bytes();
    let selected: Vec<_> = byte_pos
        .iter()
        .cloned()
        // bytes.get获得的是&u8，最后组装是&Vec<&u8>，而下面String::from_utf8_lossy要的是&[u8]
        // 所以copied
        .flat_map(|range| range.filter_map(|i| bytes.get(i).copied()))
        .collect();
    // Vec<Char>, 可直接String，但bytes不行，所以需要加一手
    // 这里正常返回的是个enum Cow
    // 需要的是String, 所以要添加into_owned, 这里解释不太够TODO:
    String::from_utf8_lossy(&selected).into_owned()
}

/*fn extract_fields(
    record: &StringRecord,
    field_pos: &[Range<usize>]
    ) -> Vec<String> {
    field_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| record.get(i)))
        // Use Iterator::map to turn &str values into String values.
        // &str转String
        .map(String::from)
        .collect()
}*/
fn extract_fields<'a>(
    record: &'a StringRecord,
    field_pos: &[Range<usize>]
    // 如果直接返回&str
    // 由于带了个borrowed type， 需要标注lifetime，不知道哪借的，recor还是field_pos
    // rust给出的建议所有给了'a生命周期，没有必要，这个生命周期只从record里借
    ) -> Vec<&'a str> {
    field_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| record.get(i)))
        .collect()
}


fn open(filename: &str) -> MyResult<Box<dyn BufRead>> {
    match filename {
        "-" => Ok(Box::new(BufReader::new(io::stdin()))),
        _ => Ok(Box::new(BufReader::new(File::open(filename)?))),
    }
}


pub fn run(config: Config) -> MyResult<()> {
    // println!("{:#?}", &config);
    for filename in &config.files {
        match open(filename) {
            Err(err) => eprintln!("{}: {}", filename, err),
            Ok(file) => match &config.extract {
                Fields(field_pos) => {
                    let mut reader = ReaderBuilder::new()
                        .delimiter(config.delimiter)
                        // 不把第一行当头处理，不然还要额外处理头
                        .has_headers(false)
                        .from_reader(file);

                    // 为了在输出中也正确转义分隔符，需要一个writer
                    let mut wtr = WriterBuilder::new()
                        .delimiter(config.delimiter)
                        .from_writer(io::stdout());

                    for record in reader.records() {
                        let record = record?;
                        // 写入record
                        wtr.write_record(extract_fields(
                                &record, field_pos,
                        ))?;
                    }
                }
                Bytes(byte_pos) => {
                    for line in file.lines() {
                        println!("{}", extract_bytes(&line?, byte_pos));
                    }
                }
                Chars(char_pos) => {
                    for line in file.lines() {
                        println!("{}", extract_chars(&line?, char_pos));
                    }
                }
            },
        }
    }
    Ok(())
}


#[cfg(test)]
mod unit_tests {
    use super::{extract_bytes, extract_chars, extract_fields, parse_pos};
    use csv::StringRecord;

    #[test]
    fn test_parse_pos() {
        // The empty string is an error
        assert!(parse_pos("").is_err());

        // Zero is an error
        let res = parse_pos("0");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"0\"",);

        let res = parse_pos("0-1");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"0\"",);

        // A leading "+" is an error
        let res = parse_pos("+1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "illegal list value: \"+1\"",
        );

        let res = parse_pos("+1-2");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "illegal list value: \"+1-2\"",
        );

        let res = parse_pos("1-+2");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "illegal list value: \"1-+2\"",
        );

        // Any non-number is an error
        let res = parse_pos("a");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"a\"",);

        let res = parse_pos("1,a");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"a\"",);

        let res = parse_pos("1-a");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "illegal list value: \"1-a\"",
        );

        let res = parse_pos("a-1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "illegal list value: \"a-1\"",
        );

        // Wonky ranges
        let res = parse_pos("-");
        assert!(res.is_err());

        let res = parse_pos(",");
        assert!(res.is_err());

        let res = parse_pos("1,");
        assert!(res.is_err());

        let res = parse_pos("1-");
        assert!(res.is_err());

        let res = parse_pos("1-1-1");
        assert!(res.is_err());

        let res = parse_pos("1-1-a");
        assert!(res.is_err());

        // First number must be less than second
        let res = parse_pos("1-1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "First number in range (1) must be lower than second number (1)"
        );

        let res = parse_pos("2-1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "First number in range (2) must be lower than second number (1)"
        );

        // All the following are acceptable
        let res = parse_pos("1");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1]);

        let res = parse_pos("01");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1]);

        let res = parse_pos("1,3");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 2..3]);

        let res = parse_pos("001,0003");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 2..3]);

        let res = parse_pos("1-3");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..3]);

        let res = parse_pos("0001-03");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..3]);

        let res = parse_pos("1,7,3-5");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 6..7, 2..5]);

        let res = parse_pos("15,19-20");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![14..15, 18..20]);
    }

    #[test]
    fn test_extract_chars() {
        assert_eq!(extract_chars("", &[0..1]), "".to_string());
        assert_eq!(extract_chars("ábc", &[0..1]), "á".to_string());
        assert_eq!(extract_chars("ábc", &[0..1, 2..3]), "ác".to_string());
        assert_eq!(extract_chars("ábc", &[0..3]), "ábc".to_string());
        assert_eq!(extract_chars("ábc", &[2..3, 1..2]), "cb".to_string());
        assert_eq!(
            extract_chars("ábc", &[0..1, 1..2, 4..5]),
            "áb".to_string()
        );
    }

    #[test]
    fn test_extract_bytes() {
        assert_eq!(extract_bytes("ábc", &[0..1]), "�".to_string()); 
        assert_eq!(extract_bytes("ábc", &[0..2]), "á".to_string());
        assert_eq!(extract_bytes("ábc", &[0..3]), "áb".to_string());
        assert_eq!(extract_bytes("ábc", &[0..4]), "ábc".to_string());
        assert_eq!(extract_bytes("ábc", &[3..4, 2..3]), "cb".to_string());
        assert_eq!(extract_bytes("ábc", &[0..2, 5..6]), "á".to_string());
    }

    #[test]
    fn test_extract_fields() {
        let rec = StringRecord::from(vec!["Captain", "Sham", "12345"]);
        assert_eq!(extract_fields(&rec, &[0..1]), &["Captain"]);
        assert_eq!(extract_fields(&rec, &[1..2]), &["Sham"]);
        assert_eq!(
            extract_fields(&rec, &[0..1, 2..3]),
            &["Captain", "12345"]
            );
        assert_eq!(extract_fields(&rec, &[0..1, 3..4]), &["Captain"]);
        assert_eq!(extract_fields(&rec, &[1..2, 0..1]), &["Sham", "Captain"]);
    }
}


