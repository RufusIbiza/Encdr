//! Maschine Mk3 Reddit Reader Demo
//!
//! Demonstrates using encdr and encdr-view to turn the Maschine Mk3 into a hardware Reddit browser:
//! - Left Screen: Smoothly scrolls real Reddit front page posts with the large 4-way encoder
//!   with fluid 60 FPS easing / tweening transitions (1 click = 1 post).
//! - Right Screen: Reads the selected post with large high-contrast typography, smoothly scrolled using Knob 5.
//! - Directional D-pad buttons (Up/Down) & Arrows also navigate posts.
//! - Encoder Press: Toggles upvote on the selected post (+1 score, pad 1 orange glow).
//! - Top Buttons 1-4: Switch feed categories with hardware LED indicators.
//!
//! Run with: cargo run --bin mk3_reddit
//! Show desktop debug windows: cargo run --bin mk3_reddit -- --visible

use std::time::{Duration, Instant};

use encdr::{Encdr, EncdrConfig, Event, LedValue};
use encdr_view::{ScreenContent, ScreenView};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Comment {
    author: String,
    score: String,
    time_ago: String,
    text: String,
    is_op: bool,
    is_nested: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RedditPost {
    id: String,
    title: String,
    subreddit: String,
    author: String,
    score: i32,
    score_short: String,
    score_full: String,
    time_ago: String,
    comments_count: i32,
    flair: Option<String>,
    #[serde(default)]
    body_html: String,
    #[serde(default)]
    comments: Vec<Comment>,
}

fn load_initial_posts() -> Vec<RedditPost> {
    let json_data = include_str!("../../screens/actual_reddit_posts.json");
    serde_json::from_str::<Vec<RedditPost>>(json_data).unwrap_or_default()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    let visible = std::env::args().any(|a| a == "--visible");

    println!("==================================================");
    println!("     Maschine Mk3 Reddit Browser Demo (Encdr)     ");
    println!("==================================================");
    println!("Controls:");
    println!("  - Large 4-Way Encoder: Turn to smoothly scroll posts (fluid sliding indicator)");
    println!("  - Directional D-Pad (Up/Down) & Arrows: Step previous/next post");
    println!("  - Encoder Press: Toggle Upvote (+1, orange pad glow)");
    println!("  - Knob 5 (screen_encoder_5): Smoothly scroll post content with inertia");
    println!("  - Top Buttons 1-4: Switch feed categories");
    println!("--------------------------------------------------");

    let mut encdr = Encdr::new(EncdrConfig::default()).expect("Failed to initialize encdr");

    println!("Scanning for Maschine Mk3...");
    let ids = encdr.scan().expect("Device scan failed");
    if ids.is_empty() {
        eprintln!("No devices found. Is your Maschine Mk3 plugged in?");
        return;
    }

    let device_id = ids
        .into_iter()
        .find(|&id| {
            encdr
                .device_descriptor(id)
                .map(|d| d.vendor_id.0 == 0x17cc && d.product_id.0 == 0x1600)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            eprintln!("Maschine Mk3 (17cc:1600) not found among connected devices.");
            std::process::exit(1);
        });

    let desc = encdr.device_descriptor(device_id).unwrap().clone();
    println!("Connected to: {}", desc.name);

    // Initial LED State: Turn on Top 1 LED (default tab) and arrow lights
    encdr.set_led(device_id, "top_1", LedValue::Single(127));
    encdr.set_led(device_id, "top_2", LedValue::Off);
    encdr.set_led(device_id, "top_3", LedValue::Off);
    encdr.set_led(device_id, "top_4", LedValue::Off);
    encdr.set_led(device_id, "arrow_left", LedValue::Single(100));
    encdr.set_led(device_id, "arrow_right", LedValue::Single(100));

    // Turn off pads
    for i in 1..=16 {
        encdr.set_led(device_id, &format!("pad_{}", i), LedValue::Off);
    }

    std::thread::sleep(Duration::from_millis(250));

    // Create Left & Right Screen Views
    let left_html = include_str!("../../screens/mk3_reddit_left.html");
    let right_html = include_str!("../../screens/mk3_reddit_right.html");

    let left_view = match ScreenView::new(
        &encdr,
        device_id,
        "left",
        ScreenContent::Html(left_html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("✓ Left Screen (Reddit Feed) initialized");
            Some(v)
        }
        Err(e) => {
            eprintln!("✗ Left Screen failed: {}", e);
            None
        }
    };

    let right_view = match ScreenView::new(
        &encdr,
        device_id,
        "right",
        ScreenContent::Html(right_html.to_string()),
        visible,
    ) {
        Ok(v) => {
            println!("✓ Right Screen (Post Reader) initialized");
            Some(v)
        }
        Err(e) => {
            eprintln!("✗ Right Screen failed: {}", e);
            None
        }
    };

    let mut posts = load_initial_posts();
    let events = encdr.events().clone();
    let mut selected_idx = 0usize;
    let mut visual_pos = 0.0f32;
    let mut upvoted = false;
    let mut dirty_left = true;
    let mut dirty_right = true;
    let mut initialized = false;

    println!("\nReddit Browser Active! Turn the large encoder to begin.\n");

    loop {
        let loop_start = Instant::now();
        ScreenView::pump_events();

        let total_posts = posts.len();

        if !initialized && total_posts > 0 {
            if let Some(ref view) = left_view {
                view.send(
                    "init_feed",
                    json!({
                        "posts": posts,
                        "selected_index": selected_idx,
                        "feed_title": "r/popular • Front Page",
                    }),
                );
            }
            if let Some(ref view) = right_view {
                view.send("load_post", json!(posts[selected_idx]));
            }
            initialized = true;
            dirty_left = true;
            dirty_right = true;
        }

        let mut post_selection_changed = false;

        while let Ok(event) = events.try_recv() {
            match event {
                // Large 4-Way Push Encoder turning: navigate posts smoothly with sliding indicator
                Event::Encoder { name, delta, .. } => {
                    if name == "main_encoder" || name == "encoder_main" {
                        let prev_idx = selected_idx;
                        if delta > 0 {
                            selected_idx = (selected_idx + 1).min(total_posts.saturating_sub(1));
                        } else if delta < 0 {
                            selected_idx = selected_idx.saturating_sub(1);
                        }

                        if selected_idx != prev_idx {
                            upvoted = false;
                            post_selection_changed = true;
                            println!(
                                "Selected Post #{}: {}",
                                selected_idx + 1,
                                posts[selected_idx].title
                            );
                        }
                    }
                }

                // Knob 5 (screen_encoder_5): smooth scroll post content
                Event::EncoderFine { name, delta, .. } => {
                    if name == "screen_encoder_5" || name == "screen_encoder_4" {
                        let pixel_delta = (delta as f64) * 1500.0;
                        if let Some(ref view) = right_view {
                            view.send("scroll_delta", json!({ "delta": pixel_delta }));
                            dirty_right = true;
                        }
                    }
                }

                // Buttons: D-Pad, Arrow buttons, Encoder click, Top category buttons
                Event::Button { name, pressed, .. } => {
                    if pressed {
                        match name {
                            // D-Pad Down / Next Post
                            "encoder_down" | "arrow_right" => {
                                if selected_idx + 1 < total_posts {
                                    selected_idx += 1;
                                    upvoted = false;
                                    post_selection_changed = true;
                                }
                            }

                            // D-Pad Up / Prev Post
                            "encoder_up" | "arrow_left" => {
                                if selected_idx > 0 {
                                    selected_idx -= 1;
                                    upvoted = false;
                                    post_selection_changed = true;
                                }
                            }

                            // Encoder Press: Upvote
                            "encoder_press" | "encoder_main_press" => {
                                upvoted = !upvoted;
                                let diff = if upvoted { 1 } else { -1 };
                                posts[selected_idx].score += diff;
                                let new_score_str = format!("{:.1}k", posts[selected_idx].score as f32 / 1000.0);

                                if let Some(ref view) = left_view {
                                    view.send(
                                        "update_score",
                                        json!({
                                            "index": selected_idx,
                                            "new_score": new_score_str,
                                        }),
                                    );
                                    dirty_left = true;
                                }
                                if let Some(ref view) = right_view {
                                    view.send("load_post", json!(posts[selected_idx]));
                                    dirty_right = true;
                                }

                                if upvoted {
                                    encdr.set_led(device_id, "pad_1", LedValue::Single(0x40));
                                } else {
                                    encdr.set_led(device_id, "pad_1", LedValue::Off);
                                }
                                println!("Upvoted: {} (Score: {})", upvoted, posts[selected_idx].score);
                            }

                            // Top 1-4 Category filter buttons
                            "top_1" => {
                                encdr.set_led(device_id, "top_1", LedValue::Single(127));
                                encdr.set_led(device_id, "top_2", LedValue::Off);
                                encdr.set_led(device_id, "top_3", LedValue::Off);
                                encdr.set_led(device_id, "top_4", LedValue::Off);
                                posts = load_initial_posts();
                                selected_idx = 0;
                                visual_pos = 0.0;
                                if let Some(ref view) = left_view {
                                    view.send(
                                        "init_feed",
                                        json!({
                                            "posts": posts,
                                            "selected_index": 0,
                                            "feed_title": "r/popular • Front Page",
                                        }),
                                    );
                                    dirty_left = true;
                                }
                                if let Some(ref view) = right_view {
                                    view.send("reset_scroll", json!({}));
                                    view.send("load_post", json!(posts[0]));
                                    dirty_right = true;
                                }
                            }
                            "top_2" => {
                                encdr.set_led(device_id, "top_1", LedValue::Off);
                                encdr.set_led(device_id, "top_2", LedValue::Single(127));
                                encdr.set_led(device_id, "top_3", LedValue::Off);
                                encdr.set_led(device_id, "top_4", LedValue::Off);
                                let all = load_initial_posts();
                                posts = all.into_iter().filter(|p| p.subreddit == "r/technology").collect();
                                selected_idx = 0;
                                visual_pos = 0.0;
                                if !posts.is_empty() {
                                    if let Some(ref view) = left_view {
                                        view.send(
                                            "init_feed",
                                            json!({
                                                "posts": posts,
                                                "selected_index": 0,
                                                "feed_title": "r/technology • Hot",
                                            }),
                                        );
                                        dirty_left = true;
                                    }
                                    if let Some(ref view) = right_view {
                                        view.send("reset_scroll", json!({}));
                                        view.send("load_post", json!(posts[0]));
                                        dirty_right = true;
                                    }
                                }
                            }
                            "top_3" => {
                                encdr.set_led(device_id, "top_1", LedValue::Off);
                                encdr.set_led(device_id, "top_2", LedValue::Off);
                                encdr.set_led(device_id, "top_3", LedValue::Single(127));
                                encdr.set_led(device_id, "top_4", LedValue::Off);
                                let all = load_initial_posts();
                                posts = all.into_iter().filter(|p| p.subreddit == "r/MadeMeSmile" || p.subreddit == "r/blender").collect();
                                selected_idx = 0;
                                visual_pos = 0.0;
                                if !posts.is_empty() {
                                    if let Some(ref view) = left_view {
                                        view.send(
                                            "init_feed",
                                            json!({
                                                "posts": posts,
                                                "selected_index": 0,
                                                "feed_title": "Wholesome & Art",
                                            }),
                                        );
                                        dirty_left = true;
                                    }
                                    if let Some(ref view) = right_view {
                                        view.send("reset_scroll", json!({}));
                                        view.send("load_post", json!(posts[0]));
                                        dirty_right = true;
                                    }
                                }
                            }
                            "top_4" => {
                                encdr.set_led(device_id, "top_1", LedValue::Off);
                                encdr.set_led(device_id, "top_2", LedValue::Off);
                                encdr.set_led(device_id, "top_3", LedValue::Off);
                                encdr.set_led(device_id, "top_4", LedValue::Single(127));
                                let all = load_initial_posts();
                                posts = all.into_iter().filter(|p| p.subreddit == "r/mildlyinfuriating" || p.subreddit == "r/confession").collect();
                                selected_idx = 0;
                                visual_pos = 0.0;
                                if !posts.is_empty() {
                                    if let Some(ref view) = left_view {
                                        view.send(
                                            "init_feed",
                                            json!({
                                                "posts": posts,
                                                "selected_index": 0,
                                                "feed_title": "Stories & Infuriating",
                                            }),
                                        );
                                        dirty_left = true;
                                    }
                                    if let Some(ref view) = right_view {
                                        view.send("reset_scroll", json!({}));
                                        view.send("load_post", json!(posts[0]));
                                        dirty_right = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // When selected post changes, update Right Screen immediately
        if post_selection_changed {
            if let Some(ref view) = right_view {
                view.send("reset_scroll", json!({}));
                view.send("load_post", json!(posts[selected_idx]));
                dirty_right = true;
            }
        }

        // Smoothly interpolate visual_pos toward selected_idx at 60 FPS
        let diff = (selected_idx as f32) - visual_pos;
        if diff.abs() > 0.004 {
            visual_pos += diff * 0.28;
            if let Some(ref view) = left_view {
                view.send(
                    "set_pos",
                    json!({
                        "selected_index": selected_idx,
                        "visual_pos": visual_pos,
                    }),
                );
                dirty_left = true;
            }
        } else if visual_pos != selected_idx as f32 {
            visual_pos = selected_idx as f32;
            if let Some(ref view) = left_view {
                view.send(
                    "set_pos",
                    json!({
                        "selected_index": selected_idx,
                        "visual_pos": visual_pos,
                    }),
                );
                dirty_left = true;
            }
        }

        // Render frames if marked dirty
        if dirty_left {
            if let Some(ref view) = left_view {
                ScreenView::pump_events();
                let _ = view.capture_and_submit(&encdr);
            }
            dirty_left = false;
        }

        if dirty_right {
            if let Some(ref view) = right_view {
                ScreenView::pump_events();
                let _ = view.capture_and_submit(&encdr);
            }
            dirty_right = false;
        }

        let elapsed = loop_start.elapsed();
        let frame_target = Duration::from_millis(16); // 60 FPS target
        if elapsed < frame_target {
            std::thread::sleep(frame_target - elapsed);
        }
    }
}
