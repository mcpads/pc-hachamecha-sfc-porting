use std::collections::BTreeMap;

pub(super) fn score(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        if !matches!(line.as_bytes(), [b'A'..=b'H', b' ', ..]) || !line.contains(" @") {
            output.push_str(line);
            output.push('\n');
            continue;
        }
        let mut split = line.split(" @");
        let prefix = split.next().unwrap();
        let notes: Vec<String> = split.map(|s| format!("@{s}")).collect();
        let mut indices: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, note) in notes.iter().enumerate() {
            indices.entry(note).or_default().push(i);
        }
        let mut parts = vec![prefix.to_owned()];
        let mut i = 0;
        while i < notes.len() {
            let mut best = (0, 0, 0);
            for &j in &indices[notes[i].as_str()] {
                if j <= i {
                    continue;
                }
                let period = j - i;
                if period > 256 {
                    break;
                }
                let mut count = 1;
                while count < 255
                    && i + (count + 1) * period <= notes.len()
                    && notes[i..i + period] == notes[i + count * period..i + (count + 1) * period]
                {
                    count += 1;
                }
                if count > 1 {
                    let size: usize = notes[i..i + period].iter().map(|s| s.len() + 1).sum();
                    let saved = (size * (count - 1)).saturating_sub(6);
                    if saved > best.0 {
                        best = (saved, period, count);
                    }
                }
            }
            let (_, period, count) = best;
            if period > 0 {
                parts.push(format!("[ {} ]{count}", notes[i..i + period].join(" ")));
                i += period * count;
            } else {
                parts.push(notes[i].clone());
                i += 1;
            }
        }
        output.push_str(&parts.join(" "));
        output.push('\n');
    }
    output
}
