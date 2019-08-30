use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use std::net::IpAddr;
use world::{Path, World, ASN};

#[derive(Clone)]
pub struct View {
    world: Rc<World>,
    perspectives: Vec<IpAddr>,
    hard_core: HashMap<(IpAddr, u32), HashSet<ASN>>,
    all_seen: HashMap<(IpAddr, u32), HashSet<ASN>>,
}

impl View {
    pub fn new(world: Rc<World>) -> Self {
        View {
            world: world,
            perspectives: vec![],
            hard_core: HashMap::new(),
            all_seen: HashMap::new(),
        }
    }

    pub fn add_perspectives(&mut self, perspectives: Vec<IpAddr>) {
        for x in perspectives.iter() {
            self.score_paths(&x);
        }
        self.perspectives.extend(perspectives);
    }

    pub fn core_dissimilarity(&self, other: &View) -> Option<f64> {
        if !Rc::ptr_eq(&self.world, &other.world) {
            return None;
        }
        let mut total: f64 = 0.0;
        let mut total_count = 0;
        for (key, count) in self.world.destination_counts.iter() {
            let mine_empty = HashSet::new();
            let their_empty = HashSet::new();
            let mine = self.hard_core.get(key).unwrap_or(&mine_empty);
            let theirs = other.hard_core.get(key).unwrap_or(&their_empty);
            let numerator = mine.difference(theirs).count() + theirs.difference(mine).count();
            let denomenator = mine.len() + theirs.len();
            if denomenator == 0 {
                continue
            }
            total_count += count;
            total += (*count as f64) * (numerator as f64) / (denomenator as f64);
        }
        total /= total_count as f64;
        return Some(total);
    }

    pub fn jaccard_dissimilarity(&self, other: &View) -> Option<f64> {
        if !Rc::ptr_eq(&self.world, &other.world) {
            return None;
        }
        let mut total: f64 = 0.0;
        let mut total_count = 0;
        for (key, count) in self.world.destination_counts.iter() {
            let mine_empty = HashSet::new();
            let their_empty = HashSet::new();
            let mine = self.all_seen.get(key).unwrap_or(&mine_empty);
            let theirs = other.all_seen.get(key).unwrap_or(&their_empty);
            let numerator = mine.intersection(theirs).count();
            let denomenator = mine.union(theirs).count();
            if denomenator == 0 {
                continue
            }
            total_count += count;
            total += (*count as f64) * (numerator as f64) / (denomenator as f64);
        }
        total /= total_count as f64;
        return Some(1.0 - total);
    }

    pub fn hard_core_mean(&self) -> f64 {
        let mut total: f64 = 0.0;
        let mut total_count = 0;
        for set in self.hard_core.values() {
            total_count += 1;
            total += set.len() as f64;
        }
        return total / (total_count as f64);
    }

    pub fn all_seen_mean(&self) -> f64 {
        let mut total: f64 = 0.0;
        let mut total_count = 0;
        for set in self.all_seen.values() {
            total_count += 1;
            total += set.len() as f64;
        }
        return total / (total_count as f64);
    }

    fn score_paths(&mut self, addr: &IpAddr) {
        for (path, ip, prefix) in self.build_paths(addr) {
            let mut value = HashSet::new();
            value.extend(path.path.clone());
            match self.hard_core.entry((ip, prefix)) {
                Vacant(vacant) => {
                    vacant.insert(value);
                }
                Occupied(mut occupied) => {
                    occupied.get_mut().intersection(&value);
                }
            }
            let mut value2 = HashSet::new();
            value2.extend(path.path);
            match self.all_seen.entry((ip, prefix)) {
                Vacant(vacant) => {
                    vacant.insert(value2);
                }
                Occupied(mut occupied) => {
                    occupied.get_mut().union(&value2);
                }
            }
        }
    }

    fn build_paths(&self, addr: &IpAddr) -> Vec<(Path, IpAddr, u32)> {
        let mut result = vec![];
        let lookup = match addr {
            IpAddr::V4(v4) => self.world.paths_v4.longest_match(*v4).map(|x| x.2),
            IpAddr::V6(v6) => self.world.paths_v6.longest_match(*v6).map(|x| x.2),
        };
        if let Some(source_known_paths_in) = lookup {
            for ((dest_addr, prefix), _) in self.world.destination_counts.iter() {
                let dest_lookup = match dest_addr {
                    IpAddr::V4(v4) => self.world.paths_v4.exact_match(*v4, *prefix),
                    IpAddr::V6(v6) => self.world.paths_v6.exact_match(*v6, *prefix),
                };
                if let Some(dest_known_paths_in) = dest_lookup {
                    if let Some(shortest) =
                        View::shortest_path(source_known_paths_in, dest_known_paths_in, &self.world)
                    {
                        result.push((shortest, *dest_addr, *prefix));
                    }
                }
            }
        }
        return result;
    }

    fn shortest_path(
        src_in_paths: &HashSet<Path>,
        dst_in_paths: &HashSet<Path>,
        world: &World,
    ) -> Option<Path> {
        let mut shortest: Option<Path> = None;
        let mut shortest_valleyless: Option<Path> = None;
        for src_path in src_in_paths.iter() {
            for dst_path in dst_in_paths.iter() {
                if let Some(short) = View::intersect_paths(src_path, dst_path) {
                    if short.valleyless(world) {
                        if shortest_valleyless.is_none() {
                            shortest_valleyless = Some(short);
                        } else {
                            shortest_valleyless = shortest_valleyless.min(Some(short));
                        }
                    } else {
                        if shortest.is_none() {
                            shortest = Some(short);
                        } else {
                            shortest = shortest.min(Some(short));
                        }
                    }
                }
            }
        }
        shortest_valleyless.or(shortest)
    }

    fn choose_best_option(pair: (usize, usize)) -> impl FnOnce((usize, usize)) -> (usize, usize) {
        move |current| {
            if current.0 + current.1 > pair.0 + pair.1 {
                current
            } else {
                pair
            }
        }
    }

    fn find_branching_point(a: &Vec<ASN>, b: &Vec<ASN>) -> Option<(usize, usize)> {
        let mut ret: Option<(usize, usize)> = None;
        for (ai, asn) in a.iter().enumerate() {
            if let Some(bi) = b.iter().rev().position(|x| x == asn) {
                let default = (ai, b.len() - 1 - bi);
                let chooser = View::choose_best_option(default);
                ret = Some(ret.map_or(default, chooser));
            }
        }
        ret
    }

    fn intersect_paths(src_in_path: &Path, dst_in_path: &Path) -> Option<Path> {
        let a = &src_in_path.path;
        let b = &dst_in_path.path;
        if let Some(branches) = View::find_branching_point(&a, &b) {
            let (_, suba) = a.split_at(branches.0);
            let (_, subb) = b.split_at(branches.1 + 1);
            let mut ret = Vec::new();
            for n in (0..suba.len()).rev() {
                ret.push(suba[n])
            }
            ret.extend_from_slice(subb);
            Some(Path { path: ret })
        } else {
            None
        }
    }
}
