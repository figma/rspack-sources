//! Tests that CachedSource produces deterministic stream_chunks output.
//!
//! The fix in cached_source.rs changes the uncached path to swallow callbacks
//! during the initial stream_and_get_source_and_map() and replay from the
//! cached map. These tests verify that:
//!
//! 1. CachedSource produces identical sources/mappings/content whether the
//!    cache is cold or warm
//! 2. Wrapping a ConcatSource of SourceMapSources in CachedSource preserves
//!    all source entries (no silently dropped segments)
//! 3. Multiple CachedSource instances over the same inner source produce
//!    identical output

use rspack_sources::{
  CachedSource, ConcatSource, MapOptions, ObjectPool, Source, SourceExt,
  SourceMap, SourceMapSource, WithoutOriginalOptions,
};
use std::sync::Arc;

/// Helper: create a SourceMapSource with given code and source path.
fn make_sms(code: &str, path: &str) -> SourceMapSource {
  let source_map = SourceMap::new(
    "AAAA",
    vec![path.to_string()],
    vec![Arc::from(code)],
    Vec::<String>::new(),
  );
  SourceMapSource::new(WithoutOriginalOptions {
    value: code.to_string(),
    name: path.to_string(),
    source_map,
  })
}

/// Helper: build a ConcatSource from multiple SourceMapSources.
fn make_concat() -> ConcatSource {
  let mut concat = ConcatSource::default();
  concat.add(make_sms("var a = 1;\n", "src/module_a.js"));
  concat.add(make_sms("var b = 2;\n", "src/module_b.js"));
  concat.add(make_sms("var c = 3;\n", "src/module_c.js"));
  concat
}

#[test]
fn cached_source_preserves_all_sources_from_concat() {
  // Build a ConcatSource with multiple SourceMapSources (simulating
  // how rspack combines modules into a chunk).
  let concat = make_concat();

  let pool = ObjectPool::default();
  let opts = MapOptions::default();

  // Get the "ground truth" map from ConcatSource directly
  let direct_map = concat.map(&pool, &opts).expect("concat should produce map");
  let direct_code = String::from_utf8_lossy(&concat.buffer()).to_string();

  // Now wrap in CachedSource and collect (cold cache)
  let cached = CachedSource::new(concat.boxed());
  let cached_map = cached.map(&pool, &opts).expect("cached should produce map");
  let cached_code = String::from_utf8_lossy(&cached.buffer()).to_string();

  // Verify all sources are preserved
  assert_eq!(
    direct_map.sources().len(),
    cached_map.sources().len(),
    "CachedSource must preserve all source entries from ConcatSource. \
     Direct had {} sources, CachedSource had {}.",
    direct_map.sources().len(),
    cached_map.sources().len(),
  );

  assert_eq!(
    direct_map.sources(),
    cached_map.sources(),
    "Source paths must match between direct and cached"
  );

  assert_eq!(
    direct_map.sources_content(),
    cached_map.sources_content(),
    "Source content must match between direct and cached"
  );

  assert_eq!(
    direct_map.mappings(),
    cached_map.mappings(),
    "Mappings must match between direct and cached"
  );

  assert_eq!(
    direct_code, cached_code,
    "Rendered code must be identical"
  );
}

#[test]
fn cached_source_cold_and_warm_produce_identical_maps() {
  let cached = CachedSource::new(make_concat().boxed());

  let pool = ObjectPool::default();
  let opts = MapOptions::default();

  // First call — cold cache
  let map1 = cached.map(&pool, &opts);
  // Second call — warm cache
  let map2 = cached.map(&pool, &opts);

  let m1 = map1.expect("cold cache should produce a map");
  let m2 = map2.expect("warm cache should produce a map");

  assert_eq!(
    m1.mappings(),
    m2.mappings(),
    "Mappings must be identical between cold and warm cache"
  );
  assert_eq!(
    m1.sources(),
    m2.sources(),
    "Sources must be identical between cold and warm cache"
  );
  assert_eq!(
    m1.sources_content(),
    m2.sources_content(),
    "Sources content must be identical between cold and warm cache"
  );
  assert_eq!(
    m1.names(),
    m2.names(),
    "Names must be identical between cold and warm cache"
  );
}

#[test]
fn multiple_cached_sources_same_inner_are_deterministic() {
  // Create two separate CachedSource instances wrapping identical content.
  // Both should produce identical output.
  for _ in 0..10 {
    let cached_a = CachedSource::new(make_concat().boxed());
    let cached_b = CachedSource::new(make_concat().boxed());

    let pool = ObjectPool::default();
    let opts = MapOptions::default();

    let map_a = cached_a.map(&pool, &opts).expect("should produce map");
    let map_b = cached_b.map(&pool, &opts).expect("should produce map");

    assert_eq!(map_a.sources(), map_b.sources());
    assert_eq!(map_a.mappings(), map_b.mappings());
    assert_eq!(map_a.sources_content(), map_b.sources_content());
  }
}

#[test]
fn cached_source_with_many_modules_preserves_all() {
  // Simulate a larger chunk with 20 modules — closer to real-world usage
  // where the race is more likely to manifest.
  let mut concat = ConcatSource::default();
  for i in 0..20 {
    concat.add(make_sms(
      &format!("var mod{i} = require('./dep{i}');\n"),
      &format!("src/module_{i}.js"),
    ));
  }

  let pool = ObjectPool::default();
  let opts = MapOptions::default();

  let direct_map = concat.map(&pool, &opts).expect("should produce map");
  let cached = CachedSource::new(concat.boxed());
  let cached_map = cached.map(&pool, &opts).expect("should produce map");

  assert_eq!(
    direct_map.sources().len(),
    20,
    "Should have all 20 source entries"
  );
  assert_eq!(
    direct_map.sources(),
    cached_map.sources(),
    "All 20 sources must be preserved through CachedSource"
  );
  assert_eq!(direct_map.mappings(), cached_map.mappings());
  assert_eq!(direct_map.sources_content(), cached_map.sources_content());
}
