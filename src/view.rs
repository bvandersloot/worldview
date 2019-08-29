use std::collections::HashSet;
use std::rc::Rc;

use world::{World,Path,ASN};
use std::net::{IpAddr};

pub struct View {
    world: Rc<World>,
    perspectives: Vec<IpAddr>,
    counts: Vec<u64>,
}

impl View {
    pub fn new(world : Rc<World>) -> Self {
        let counts = vec![0; world.known_asns.len()];
        View{
            world: world,
            perspectives: vec![],
            counts: counts,
        }
    }

    pub fn add_perspectives(&mut self, perspectives : Vec<IpAddr>) {
        for x in perspectives.iter() {
            self.score_paths(&x);
        }
        self.perspectives.extend(perspectives);
    }

    pub fn features(&self) -> &Vec<u64> {
        &self.counts
    }

    pub fn distance(&self, other: &Path) -> f64 {
        0.0
    }

    fn score_paths(&mut self, addr: &IpAddr) {
        for (path, count) in self.build_paths(addr) {
            for asn in path.path.iter() {
                if let Some(idx) = self.world.known_asns.iter().position(|&x| x == *asn) {
                    self.counts[idx] += count
                }
            }
        }
    }

    fn build_paths(&self, addr: &IpAddr) -> Vec<(Path, u64)> {
        let mut result = vec![];
        let lookup = match addr {
            IpAddr::V4(v4) => self.world.paths_v4.longest_match(*v4).map(|x| x.2),
            IpAddr::V6(v6) => self.world.paths_v6.longest_match(*v6).map(|x| x.2),
        };
        if let Some(source_known_paths_in) = lookup {
            for ((dest_addr, prefix), count) in self.world.destination_counts.iter() {
                let dest_lookup = match dest_addr {
                    IpAddr::V4(v4) => self.world.paths_v4.exact_match(*v4, *prefix),
                    IpAddr::V6(v6) => self.world.paths_v6.exact_match(*v6, *prefix),
                };
                if let Some(dest_known_paths_in) = dest_lookup {
                    if let Some(shortest) = View::shortest_path(source_known_paths_in, dest_known_paths_in, &self.world) {
                        result.push((shortest, *count));
                    }
                }
            }
        } 
        return result
    }

    fn shortest_path(src_in_paths : &HashSet<Path>, dst_in_paths : &HashSet<Path>, world: &World) -> Option<Path> {
        let mut shortest : Option<Path> = None;
        let mut shortest_valleyless : Option<Path> = None;
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
            Some(Path{path:ret})
        } else {
            None
        }
    }

}
