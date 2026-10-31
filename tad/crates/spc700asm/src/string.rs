pub fn split_first_word(s: &str) -> (&str, &str) {
    match s.split_once(|c: char| c.is_ascii_whitespace()) {
        Some((a, b)) => (a, b.trim_start()),
        None => (s, ""),
    }
}
pub struct CommaIter<'a>(&'a str);
impl<'a> Iterator for CommaIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        let s = self.0.trim_start();
        if !s.is_empty() {
            let mut in_quote = false;
            let mut in_str = false;
            let mut n_parenthesis = 0;
            for (i, c) in s.bytes().enumerate() {
                match c {
                    b',' if !in_quote && !in_str && n_parenthesis == 0 => {
                        self.0 = &s[i + 1..];
                        return Some(s[..i].trim_end());
                    }
                    b'\'' if !in_str => in_quote = !in_quote,
                    b'\"' if !in_quote => in_str = !in_str,
                    b'(' if !in_str && !in_quote => n_parenthesis += 1,
                    b')' if !in_str && !in_quote => n_parenthesis -= 1,
                    _ => {}
                }
            }
            self.0 = "";
            Some(s.trim_end())
        } else {
            None
        }
    }
}
pub fn comma_iter(s: &str) -> CommaIter<'_> {
    CommaIter(s)
}
pub fn strip_comment(s: &str) -> &str {
    let s = s.trim_start();
    if !s.is_empty() {
        let mut in_quote = false;
        let mut in_str = false;
        for (i, c) in s.bytes().enumerate() {
            match c {
                b';' if !in_quote && !in_str => return s[..i].trim_end(),
                b'\'' if !in_str => in_quote = !in_quote,
                b'\"' if !in_quote => in_str = !in_str,
                _ => {}
            }
        }
        s.trim_end()
    } else {
        ""
    }
}
