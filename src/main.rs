extern crate itertools;
extern crate serde;
extern crate treebitmap;

mod view;
mod world;

use view::View;
use world::World;

use itertools::Itertools;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::rc::Rc;
use std::str::FromStr;



fn load_views(fname: &str, world : Rc<World>) -> HashMap<String, View> {
    let mut result = HashMap::new();
    let f = File::open(fname).expect("file not found");
    for line in BufReader::new(f).lines().map(|x| x.unwrap()) {
        let tokens : Vec<&str> = line.split(",").collect();
        let addr = IpAddr::from_str(tokens[1]).unwrap();
        let mut v = View::new(world.clone());
        v.add_perspectives(vec![addr]);
        result.insert(tokens[0].to_string(), v);
    }
    result
}

fn main() {
    let w = World::build_new("../as_relationships.txt", "../bgp.txt", "../sites.txt");
    let rc_w = Rc::new(w);
    let views = load_views("../servers.txt", rc_w);
    
    println!("{}", views.len());
   
    for (name, view) in views.iter() {
        println!("{} {} {}", name, view.hard_core_mean(), view.all_seen_mean());
    }

    for ((a_name, a_view), (b_name, b_view)) in views.iter().tuple_combinations() {
        println!("{} {} {} {}", a_name, b_name, a_view.core_dissimilarity(b_view).unwrap(), a_view.jaccard_dissimilarity(b_view).unwrap());
    }
}
