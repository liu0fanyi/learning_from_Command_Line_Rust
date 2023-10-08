use clap::{App, Arg};

fn main() {
    let matches = App::new("echor")
        .version("0.1.0")
        .author("Ken Youens-Clark <kyclark@gmail.com>")
        .about("Rust echo")
        .arg(
            // Create a new Arg with the name text. This is a required positional argument that must appear at least once and can be repeated.这种arg，positional argument 至少得有一个, 因为那个required true吧
            Arg::with_name("text")
            .value_name("TEXT")
            .help("Input text")
            .required(true)
            .min_values(1),
        )
        .arg(
            Arg::with_name("omit_newline")
            .short("n")
            .help("Do not print newline")
            .takes_value(false),
        )
        .get_matches();
    
    let text = matches.values_of_lossy("text").unwrap();
    let omit_newline = matches.is_present("omit_newlne");

    print!("{}{}", text.join(" "), if omit_newline { "" } else {"\n"});
}
