//! Integration test for enrichment's cloud-safety (spec 010 AC4): a file
//! carrying the Windows offline attribute is size-only — its contents are
//! never read, so atref can't trigger OneDrive/Dropbox hydration (the
//! 2026-06-09 `.gitignore` incident must not recur through this path).

#![cfg(windows)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use atref::enrich::{enrich_file, is_cloud_placeholder, DEFAULT_CAPS};

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atref_it_enrich_{}_{name}", std::process::id()))
}

#[test]
fn offline_placeholder_is_size_only() {
    let path = tmp("offline.md");
    fs::write(&path, b"# cloud-only doc\nbody\n").unwrap();
    let status = Command::new("attrib")
        .arg("+O")
        .arg(&path)
        .status()
        .expect("attrib runs");
    assert!(status.success(), "attrib +O set the offline attribute");

    let md = fs::metadata(&path).unwrap();
    assert!(is_cloud_placeholder(&md), "offline attribute detected");

    let e = enrich_file(&path, &md, &DEFAULT_CAPS);
    assert_eq!(e.size, md.len(), "size still reported, from metadata alone");
    assert_eq!(e.lines, None, "contents not read");
    assert_eq!(e.tokens, None, "contents not read");

    // A regular sibling gets full metrics — the guard isn't over-broad.
    let plain = tmp("plain.md");
    fs::write(&plain, b"# title\nbody\n").unwrap();
    let pmd = fs::metadata(&plain).unwrap();
    assert!(!is_cloud_placeholder(&pmd));
    let pe = enrich_file(&plain, &pmd, &DEFAULT_CAPS);
    assert_eq!(pe.lines, Some(2));
    assert!(pe.tokens.unwrap() > 0);

    let _ = Command::new("attrib").arg("-O").arg(&path).status();
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&plain);
}

#[test]
fn offline_image_gets_no_thumbnail() {
    // Spec 011 AC4: a cloud-only image is never decoded — no thumb, no read.
    // A 1×1 PNG written locally, then flagged offline.
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let path = tmp("offline.png");
    fs::write(&path, png).unwrap();
    let status = Command::new("attrib")
        .arg("+O")
        .arg(&path)
        .status()
        .expect("attrib runs");
    assert!(status.success());

    let md = fs::metadata(&path).unwrap();
    assert!(is_cloud_placeholder(&md));
    let e = enrich_file(&path, &md, &DEFAULT_CAPS);
    assert_eq!(e.thumb, None, "cloud-only image is not decoded");
    assert_eq!(e.size, md.len());

    let _ = Command::new("attrib").arg("-O").arg(&path).status();
    let _ = fs::remove_file(&path);
}
