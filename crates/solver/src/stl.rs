use crate::fdtd::Triangle;

fn ascii(text: &str) -> Result<Vec<Triangle>, String> {
    let mut out = Vec::new();
    let mut corner: Vec<[f64; 3]> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        if words.next() != Some("vertex") {
            continue;
        }
        let mut p = [0.0; 3];
        for v in &mut p {
            *v = words
                .next()
                .and_then(|w| w.parse().ok())
                .ok_or_else(|| format!("line {}: a vertex needs three numbers", n + 1))?;
        }
        corner.push(p);
        if corner.len() == 3 {
            out.push([corner[0], corner[1], corner[2]]);
            corner.clear();
        }
    }
    if !corner.is_empty() {
        return Err("the last facet has fewer than three vertices".into());
    }
    Ok(out)
}

fn binary(bytes: &[u8]) -> Result<Vec<Triangle>, String> {
    let count = u32::from_le_bytes(bytes[80..84].try_into().map_err(|_| "short header")?) as usize;
    if bytes.len() < 84 + 50 * count {
        return Err(format!(
            "the file says {count} triangles but holds {}",
            (bytes.len() - 84) / 50
        ));
    }
    let float =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok((0..count)
        .map(|t| {
            let base = 84 + 50 * t + 12;
            std::array::from_fn(|v| std::array::from_fn(|a| float(base + 12 * v + 4 * a) as f64))
        })
        .collect())
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Triangle>, String> {
    let fits_binary = bytes.len() >= 84 && {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        bytes.len() == 84 + 50 * count
    };
    let triangles = if bytes.starts_with(b"solid") && !fits_binary {
        ascii(&String::from_utf8_lossy(bytes))?
    } else if bytes.len() >= 84 {
        binary(bytes)?
    } else {
        return Err("not an STL file".into());
    };
    if triangles.is_empty() {
        return Err("the STL file has no triangles".into());
    }
    if triangles.iter().flatten().flatten().any(|v| !v.is_finite()) {
        return Err("the STL file has a coordinate that is not a number".into());
    }
    Ok(triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE: [[f64; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.5]];

    #[test]
    fn ascii_and_binary_read_the_same_triangle() {
        let text = "solid t\n facet normal 0 0 1\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 2 0.5\n  endloop\n endfacet\nendsolid t\n";
        assert_eq!(parse(text.as_bytes()).unwrap(), vec![TRIANGLE]);
        let mut bin = b"solid but really binary".to_vec();
        bin.resize(80, 0);
        bin.extend(1u32.to_le_bytes());
        bin.extend([0u8; 12]);
        for p in TRIANGLE {
            for v in p {
                bin.extend((v as f32).to_le_bytes());
            }
        }
        bin.extend([0u8; 2]);
        assert_eq!(parse(&bin).unwrap(), vec![TRIANGLE]);
    }

    #[test]
    fn broken_files_say_why() {
        assert!(parse(b"solid x\nvertex 1 2\n").unwrap_err().contains("three numbers"));
        assert!(parse(b"tiny").unwrap_err().contains("not an STL"));
        assert!(parse(b"solid x\nendsolid x\n").unwrap_err().contains("no triangles"));
    }
}
