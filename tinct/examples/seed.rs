fn main() {
    let path = std::env::args().nth(1).expect("usage: seed <image>");
    let t0 = std::time::Instant::now();
    let seed = tinct::seed_from_path(&path, &Default::default()).unwrap();
    println!("seed: {}  HCT: {:.2} {:.2} {:.2}  ({:?})",
        tinct::argb_to_hex(seed.argb), seed.hue, seed.chroma, seed.tone, t0.elapsed());
}
