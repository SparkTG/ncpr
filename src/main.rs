extern crate csv;

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Result;
use std::io::Write;
use std::path::MAIN_SEPARATOR;

/*
    Files are arranged and named from first 4 digits of number.

    Data is boxed in a file. The first byte of file indicates the type. Data follows after that.
    There are two possible formats (types):
        Type 0: uncompressed 2 byte data series
        Type 1: 3 byte address, 2 byte data

    The unpacked layout for a data file is a 2,000,000 bytes array.
    Two bytes are occupied for every number.
    First byte (b1) is organised as (1 bit fill indicator, 5 bit service area code, 2 bit phone type)
    Second byte (b2) is organised as (7 bit preferences, 1 bit opstype)
*/

const DATA_DIR: &'static str = "/opt/data/ncpr";

// filename for given prefix
macro_rules! filename {
    ( $x:expr ) => { format!("{}{}{}.dat", DATA_DIR, MAIN_SEPARATOR, $x) };
}

fn serialize(s: u8, p: &str, o: &str, pt: u8) -> (u8, u8) {
    let mut b1 = 1 << 7;  // indicator bit
    let mut b2 = 0;

    assert!(s > 0 && s < 32);
    b1 |=  s << 2;

    assert!(pt > 0 && pt < 4);
    b1 |= pt;

    if p != "0" {
        for i in p.split('#').filter_map(|x| if x.is_empty() { None } else { Some(x.parse::<u8>().unwrap()) }) {
            assert!(i >= 1 && i <= 7);
            b2 |= 1 << i
        }
    }

    if o != "D" {
        assert!(o == "A");
        b2 |= 1;
    }

    (b1, b2)
}

fn deserialize(b1: u8, b2: u8) -> Option<(u8, String, String, u8)> {
    if b1 & 0b1000_0000 == 0 {
        return None;
    }

    Some((
        (b1 & 0b0111_1100) >> 2,
        if b2 & 0b1111_1110 == 0 {
            "0".to_string()
        } else {
            (1..8).filter(|&x| (b2 >> x) & 1 == 1).map(|x| x.to_string()).collect::<Vec<String>>().join("#")
        },
        if b2 & 1 == 0 {
            "D".to_string()
        } else {
            "A".to_string()
        },
        b1 & 0b0000_0011
    ))
}

fn pack_addr(j: usize) -> (u8, u8, u8) {
    (
        ((j & 0xff0000) >> 16) as u8,
        ((j & 0x00ff00) >> 8) as u8,
        (j & 0x0000ff) as u8
    )
}

fn unpack_addr(b1: u8, b2: u8, b3: u8) -> usize {
    ((b1 as usize) << 16) | ((b2 as usize) << 8) | (b3 as usize)
}

fn load(h: &u32, unpacked: &mut[u8]) -> Result<()> {
    // h is prefix (first 4 digits of phone number)
    assert!(unpacked.len() == 2_000_000);

    let filename = filename!(h);
    if fs::metadata(&filename).is_err() {
        return Ok(());
    }

    let mut f = try!(File::open(filename));
    let mut buf = Vec::with_capacity(2_000_001);
    let bytes_read = try!(f.read_to_end(&mut buf));

    if buf[0] & 1 == 0 {
        assert!((bytes_read - 1) == 2_000_000);
        for i in 1..bytes_read {
            unpacked[i - 1] = buf[i];
        }
        return Ok(());
    }

    assert!((bytes_read - 1) % 5 == 0);
    let mut i = 1;
    while i < bytes_read {
        let j = unpack_addr(buf[i], buf[i + 1], buf[i + 2]);
        unpacked[2 * j] = buf[i + 3];
        unpacked[2 * j + 1] = buf[i + 4];
        i += 5;
    }

    Ok(())
}

fn dump(h: &u32, unpacked: &[u8]) -> Result<()> {
    assert!(unpacked.len() == 2_000_000);

    let mut i = 0;
    let mut filled_count = 0;
    while i < 2_000_000 {
        if unpacked[i] & 0b1000_0000 != 0 {
            filled_count += 1;
        }
        i += 2;
    }

    let filename = filename!(h);
    let mut f = File::create(filename).unwrap();
    if filled_count >= 400_000 {  // >= 40%
        try!(f.write(&mut [0u8]));
        try!(f.write(unpacked));
        try!(f.flush());
        return Ok(());
    }

    let mut buf = vec![0u8;filled_count * 5];
    let mut j = 0;
    i = 0;
    while i < 2_000_000 {
        if unpacked[i] & 0b1000_0000 != 0 {
            let (b1, b2, b3) = pack_addr(i / 2);
            buf[j] = b1;
            buf[j + 1] = b2;
            buf[j + 2] = b3;
            buf[j + 3] = unpacked[i];
            buf[j + 4] = unpacked[i + 1];
            j += 5;
        }
        i += 2;
    }

    try!(f.write(&mut [1u8]));
    try!(f.write(&buf[..]));
    try!(f.flush());
    Ok(())
}

// search in appropriate file
fn search(pn: &str) -> Option<(u8, String, String, u8)> {
    // binary search for type 2
    fn binary_search(key: usize, data: &[u8], low: usize, high: usize) -> Option<(u8, String, String, u8)> {
        // inclusive of low and high index for searching
        if low > high {
            return None;
        }

        let mid = (low + high) / 2;
        let mid_key = unpack_addr(data[mid * 5], data[mid * 5 + 1], data[mid * 5 + 2]);

        if key < mid_key {
            binary_search(key, data, low, mid - 1)
        } else if key == mid_key {
            deserialize(data[mid * 5 + 3], data[mid * 5 + 4])
        } else {
            binary_search(key, data, mid + 1, high)
        }
    }

    assert!(pn.len() == 10);
    let h = pn[..4].parse::<u32>().unwrap();
    let t = pn[4..].parse::<usize>().unwrap();

    if t >= 1_000_000 {
        return None;
    }

    let filename = filename!(h);
    if fs::metadata(&filename).is_err() {
        return None;
    }

    let mut f = File::open(filename).unwrap();
    let mut buf = Vec::with_capacity(2_000_001);
    let bytes_read = f.read_to_end(&mut buf).unwrap();

    if buf[0] & 1 == 0 {
        assert!((bytes_read - 1) == 2_000_000);
        return deserialize(buf[2 * t + 1], buf[2 * t + 2]);
    }

    assert!((bytes_read - 1) % 5 == 0);
    binary_search(t, &buf[1..], 0, (bytes_read - 1) / 5 - 1)
}

// patch from stdin
fn patch() {
    let mut rdr = csv::Reader::from_reader(io::stdin());

    let mut head_map: HashMap<u32, Vec<(usize, (u8, u8))>> = HashMap::with_capacity(10_000);
    for record in rdr.decode() {
        let (s, pn, p, o, pt): (u8, String, String, String, u8) = record.unwrap();
        if pn.as_bytes().iter().filter(|&x| *x >= 0x30 && *x <= 0x39).count() != 10 {
            println!("ignoring invalid number {}", pn);
            continue;
        }
        let h = pn[..4].parse::<u32>().unwrap();
        let t = pn[4..].parse::<usize>().unwrap();
        if !head_map.contains_key(&h) {
            head_map.insert(h, Vec::new());
        }
        head_map.get_mut(&h).unwrap().push((t, serialize(s, &p, &o, pt)));
    }

    let total = head_map.len();
    println!("patching {} files", total);

    //let mut i = 0;
    let mut count = 0;
    for (h, items) in head_map.iter() {
        //println!("[{}/{}] processing {}", i + 1, total, h);

        let mut unpacked = [0u8;2_000_000];
        let load_result = load(h, &mut unpacked);
        assert!(load_result.is_ok());

        for &(t, (b1, b2)) in items {
            unpacked[2 * t] = b1;
            unpacked[2 * t + 1] = b2;
            count += 1;
        }

        let dump_result = dump(h, &unpacked);
        assert!(dump_result.is_ok());

        //i += 1;
    }
    println!("patched a total of {} records", count);
}

fn main() {
    let program_name = ::std::env::args().nth(0).unwrap();
    let command = ::std::env::args().nth(1);
    match command {
        Some(ref s) if *s == "patch".to_string() => {
            println!("reading from stdin");
            patch();
        },
        Some(ref s) if *s == "search".to_string() => {
            let keyword = ::std::env::args().nth(2);
            if keyword == None {
                println!("missing argument phone number");
            } else {
                println!("{:?}", search(&keyword.unwrap()));
            }
        }
        _ => {
            println!("Usage: {} [patch|[search number]]", program_name);
        }
    }
}
