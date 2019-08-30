extern crate serde;
extern crate treebitmap;

mod view;
mod world;

use view::View;
use world::World;

use std::rc::Rc;
use std::net::{IpAddr,Ipv4Addr};

fn main() {
    let w = World::build_new("../as_relationships.txt", "../bgp.txt", "../sites.txt");
    let rc_w = Rc::new(w);
    let mut v = View::new(rc_w);
    let test = IpAddr::V4(Ipv4Addr::new(141, 212, 121, 1));
    v.add_perspectives(vec![test]);
    println!("Hello, world!");
}
