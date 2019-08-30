extern crate treebitmap;

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::IpAddr::{V4, V6};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use treebitmap::IpLookupTable;

pub struct World {
    pub(crate) as_relationships: HashMap<(ASN, ASN), ASRelation>,
    pub(crate) paths_v4: IpLookupTable<Ipv4Addr, HashSet<Path>>,
    pub(crate) paths_v6: IpLookupTable<Ipv6Addr, HashSet<Path>>,
    pub(crate) destination_counts: HashMap<(IpAddr, u32), u64>,
    pub(crate) known_asns: BTreeSet<ASN>,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct Path {
    pub(crate) path: Vec<ASN>,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd)]
pub(crate) enum ASRelation {
    No,
    Consumes,
    Peers,
    Provides,
}

pub type ASN = u64;

impl World {
    pub fn build_new(
        as_relationship_file: &str,
        bgp_data_file: &str,
        destination_file: &str,
    ) -> World {
        let mut result = World {
            as_relationships: HashMap::new(),
            paths_v4: IpLookupTable::new(),
            paths_v6: IpLookupTable::new(),
            destination_counts: HashMap::new(),
            known_asns: BTreeSet::new(),
        };
        result.load_relationships(as_relationship_file);
        result.load_bgp_data(bgp_data_file);
        result.load_destinations(destination_file);
        result
    }

    fn load_relationships(&mut self, fname: &str) {
        let mut result = HashMap::new();
        let f = File::open(fname).expect("file not found");
        for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split("|").collect();
            let a = fields[0].parse::<ASN>().unwrap();
            let b = fields[1].parse::<ASN>().unwrap();
            let r = fields[2].parse::<i64>().unwrap();
            if r == -1 {
                result.insert((a, b), ASRelation::Provides);
                result.insert((b, a), ASRelation::Consumes);
            } else if r == 0 {
                result.insert((a, b), ASRelation::Peers);
                result.insert((b, a), ASRelation::Peers);
            }
        }
        self.as_relationships = result
    }

    fn load_bgp_data(&mut self, fname: &str) {
        let mut known: HashMap<String, HashSet<Path>> = HashMap::new();
        let f = File::open(fname).expect("file not found");
        for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
            let fields: Vec<&str> = line.split("|").collect();
            if fields[1] == "R" {
                for entry in fields[9].split(" ") {
                    for asn in Path::parse_str_to_asns(entry).iter() {
                        self.known_asns.insert(*asn);
                    }
                }
                let prefix = fields[7].to_string();
                let paths = Path::build_from_str(fields[9]);
                if let Some(known_paths) = known.get_mut(&prefix) {
                    known_paths.extend(paths);
                } else {
                    known.insert(prefix, paths);
                }
            }
        }
        for (prefix, set) in known.drain() {
            let split_prefix: Vec<&str> = prefix.split("/").collect();
            let addr = IpAddr::from_str(split_prefix[0]).unwrap();
            let prefix_length = split_prefix[1].parse::<u32>().expect(prefix.as_str());
            match addr {
                V4(v4) => {
                    self.paths_v4.insert(v4, prefix_length, set);
                }
                V6(v6) => {
                    self.paths_v6.insert(v6, prefix_length, set);
                }
            }
        }
    }

    fn load_destinations(&mut self, fname: &str) {
        let f = File::open(fname).expect("file not found");
        for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
            let addr = IpAddr::from_str(line.as_str()).unwrap();
            match addr {
                V4(v4) => {
                    if let Some((match_addr, match_len, _)) = self.paths_v4.longest_match(v4) {
                        *self
                            .destination_counts
                            .entry((IpAddr::V4(match_addr), match_len))
                            .or_insert(0) += 1;
                    }
                }
                V6(v6) => {
                    if let Some((match_addr, match_len, _)) = self.paths_v6.longest_match(v6) {
                        *self
                            .destination_counts
                            .entry((IpAddr::V6(match_addr), match_len))
                            .or_insert(0) += 1;
                    }
                }
            }
        }
    }
}

impl Path {
    pub fn new() -> Path {
        Path { path: vec![] }
    }

    pub fn build_from_str(path: &str) -> HashSet<Path> {
        let parsed: VecDeque<&str> = path.split(" ").collect();
        Path::build_from_vec(parsed)
    }

    fn build_from_vec(path: VecDeque<&str>) -> HashSet<Path> {
        let mut result = HashSet::new();
        let mut current_set = vec![Path::new()];
        for asn in path.iter().rev() {
            let mut new_set = vec![];
            for parsed_asn in Path::parse_str_to_asns(asn).iter() {
                for current_path in current_set.iter() {
                    let new_path = Path::prepend(&current_path, parsed_asn);
                    new_set.push(new_path);
                }
            }
            result.extend(new_set.clone());
            current_set = new_set;
        }
        result
    }

    fn prepend(path: &Path, asn: &ASN) -> Path {
        let mut new_path = vec![*asn];
        new_path.extend(path.path.clone());
        return Path { path: new_path };
    }

    fn parse_str_to_asns(string: &str) -> Vec<ASN> {
        let mut result = vec![];
        if string.contains('{') || string.contains('(') || string.contains('[') {
            let trimmed = string.trim_matches(|c| "{}()[]".contains(c));
            for asn in trimmed.split(',') {
                result.push(asn.parse::<ASN>().unwrap());
            }
        } else {
            result.push(string.parse::<ASN>().unwrap());
        }
        result
    }

    pub fn valleyless(&self, world: &World) -> bool {
        let mut state = ASRelation::No;
        for i in 0..self.path.len() - 1 {
            if let Some(next) = world
                .as_relationships
                .get(&(self.path[i], self.path[i + 1]))
            {
                if next >= &state {
                    state = *next;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

impl Ord for Path {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.len().cmp(&other.path.len())
    }
}
impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
