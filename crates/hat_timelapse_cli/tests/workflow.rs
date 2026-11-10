//! End-to-end: init → keyframe edits in sidecars → interpolate → deflicker.

use image::{Rgb, RgbImage};
use std::path::Path;
use std::process::Command;

fn hat_timelapse(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_hat_timelapse"))
        .arg("-p")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn exposure(dir: &Path, name: &str) -> f64 {
    let props = hat_timelapse_xmp_read(&dir.join(name));
    props.trim_start_matches('+').parse().unwrap()
}

fn hat_timelapse_xmp_read(path: &Path) -> String {
    let s = std::fs::read_to_string(path).unwrap();
    let k = s.find("crs:Exposure2012=\"").unwrap() + 18;
    s[k..k + s[k..].find('"').unwrap()].to_string()
}

#[test]
fn edit_loop() {
    let dir = std::env::temp_dir().join(format!("hat_timelapse-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..40u32 {
        // Mid-grey with an alternating flicker.
        let v = if i % 2 == 0 { 120 } else { 104 };
        RgbImage::from_pixel(64, 48, Rgb([v, v, v]))
            .save(dir.join(format!("f_{i:03}.png")))
            .unwrap();
    }
    hat_timelapse(&dir, &["init", "--keyframes", "2"]);

    // "Edit" the two keyframes, as Lightroom or LightCraft would.
    for (name, ev) in [("f_000.xmp", "+1.00"), ("f_039.xmp", "-1.00")] {
        let xml = hat_timelapse_xmp_write(ev);
        std::fs::write(dir.join(name), xml).unwrap();
    }
    hat_timelapse(&dir, &["interpolate", "--method", "linear"]);
    // Settings that are not animated reach the frames between keyframes.
    let mid = std::fs::read_to_string(dir.join("f_020.xmp")).unwrap();
    assert!(mid.contains(r#"crs:LensProfileEnable="1""#), "{mid}");
    assert!(mid.contains("<rdf:li>128, 140</rdf:li>"), "{mid}");
    assert!((exposure(&dir, "f_020.xmp") - (-0.03)).abs() < 0.02);

    hat_timelapse(&dir, &["deflicker", "--window", "9"]);
    // Neighbouring frames now differ only by the ramp (~0.05 EV/frame), not the flicker.
    let a = exposure(&dir, "f_020.xmp");
    let b = exposure(&dir, "f_021.xmp");
    assert!(
        (b - a).abs() > 0.2,
        "deflicker should counter the ~0.35 EV flicker: {a} {b}"
    );

    // Re-reading keyframes must recover the user's values, not the deflickered ones.
    hat_timelapse(&dir, &["interpolate"]);
    let p: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("hat_timelapse.json")).unwrap())
            .unwrap();
    let k0 = p["frames"][0]["values"]["Exposure2012"].as_f64().unwrap();
    assert!((k0 - 1.0).abs() < 0.011, "{k0}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn hat_timelapse_xmp_write(ev: &str) -> String {
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:Exposure2012="{ev}" crs:LensProfileEnable="1">
<crs:ToneCurvePV2012><rdf:Seq><rdf:li>0, 0</rdf:li><rdf:li>128, 140</rdf:li><rdf:li>255, 255</rdf:li></rdf:Seq></crs:ToneCurvePV2012>
</rdf:Description>
</rdf:RDF></x:xmpmeta>"#
    )
}

#[test]
fn lightcraft_edited_keyframes() {
    let dir = std::env::temp_dir().join(format!("hat-tl-lc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..11u32 {
        RgbImage::from_pixel(32, 24, Rgb([110, 110, 110]))
            .save(dir.join(format!("g_{i:02}.png")))
            .unwrap();
    }
    hat_timelapse(&dir, &["init", "--keyframes", "2"]);
    // As LightCraft writes them: complete settings as JSON in lc:settings, no crs: values.
    for (name, ev) in [("g_00.xmp", 1.0), ("g_10.xmp", -1.0)] {
        let json = format!(
            r#"{{"wb":{{"mode":"asShot","temp":0.0,"tint":0.0}},"light":{{"exposure":{ev},"contrast":0.0}},"curve":{{"marker":"keep-me"}}}}"#
        );
        let attr = json.replace('"', "&quot;");
        std::fs::write(
            dir.join(name),
            format!(
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:lc="http://ns.lightcraft.app/lc/1.0/" lc:settings="{attr}"/>
</rdf:RDF></x:xmpmeta>"#
            ),
        )
        .unwrap();
    }
    hat_timelapse(&dir, &["interpolate", "--method", "linear"]);
    let mid = std::fs::read_to_string(dir.join("g_05.xmp")).unwrap();
    // Exposure animated (0 midway), written for Lightroom and for LightCraft.
    assert!(mid.contains(r#"crs:Exposure2012="+0.00""#), "{mid}");
    let k = mid.find("lc:settings=\"").unwrap() + 13;
    let json: serde_json::Value =
        serde_json::from_str(&mid[k..k + mid[k..].find('"').unwrap()].replace("&quot;", "\""))
            .unwrap();
    assert!(
        json["light"]["exposure"].as_f64().unwrap().abs() < 1e-6,
        "{json}"
    );
    assert_eq!(json["curve"]["marker"], "keep-me");
    // White balance was "as shot", so no temperature is forced on the frames.
    assert!(!mid.contains("crs:Temperature"), "{mid}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn steps_in_any_order_keep_keyframe_edits() {
    let dir = std::env::temp_dir().join(format!("hat-tl-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..20u32 {
        let v = if i % 2 == 0 { 120 } else { 100 };
        RgbImage::from_pixel(32, 24, Rgb([v, v, v]))
            .save(dir.join(format!("o_{i:02}.png")))
            .unwrap();
    }
    hat_timelapse(&dir, &["init", "--keyframes", "2"]);
    let set = |ev: &str| std::fs::write(dir.join("o_00.xmp"), hat_timelapse_xmp_write(ev)).unwrap();
    let user = || -> f64 {
        let p: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("hat_timelapse.json")).unwrap())
                .unwrap();
        p["frames"][0]["values"]["Exposure2012"].as_f64().unwrap()
    };
    let written = || -> f64 {
        let p: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("hat_timelapse.json")).unwrap())
                .unwrap();
        p["frames"][0]["written_exposure"].as_f64().unwrap()
    };

    // Edit a keyframe, then deflicker before applying edits: the edit must survive.
    set("+1.50");
    hat_timelapse(&dir, &["deflicker", "--window", "5"]);
    assert!((user() - 1.5).abs() < 0.011, "{}", user());

    // The editor saves its stale value without re-reading: still +1.50, not double-corrected.
    set("+1.50");
    hat_timelapse(&dir, &["interpolate"]);
    assert!((user() - 1.5).abs() < 0.011, "{}", user());

    // The editor re-reads (sees the written value) and adds half a stop.
    set(&format!("{:+.2}", written() + 0.5));
    hat_timelapse(&dir, &["interpolate"]);
    assert!((user() - 2.0).abs() < 0.011, "{}", user());
    let _ = std::fs::remove_dir_all(&dir);
}
