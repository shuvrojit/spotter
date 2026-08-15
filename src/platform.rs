use crate::{config::UiConfig, config::WindowPosition, APP_ID};
use anyhow::{Context, Result};
use gtk::{glib, prelude::*, ApplicationWindow};
use serde::Deserialize;
use std::{env, fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
struct SwayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize)]
struct SwayWorkspace {
    focused: bool,
    rect: SwayRect,
}

#[derive(Debug, Deserialize)]
struct SwayCommandResult {
    success: bool,
    error: Option<String>,
}

pub(crate) fn schedule_position(window: &ApplicationWindow, ui: UiConfig) {
    for attempt in 1..=4 {
        let window = window.clone();
        let ui = ui.clone();
        glib::timeout_add_local_once(Duration::from_millis(120 * attempt), move || {
            apply_position(&window, &ui)
        });
    }
}

pub(crate) fn schedule_reposition(window: &ApplicationWindow, ui: UiConfig) {
    let window = window.clone();
    glib::timeout_add_local_once(Duration::from_millis(30), move || {
        apply_position(&window, &ui)
    });
}

fn apply_position(window: &ApplicationWindow, ui: &UiConfig) {
    if env::var_os("SWAYSOCK").is_some() && command_exists("swaymsg") {
        if let Err(error) = position_with_sway(window, ui) {
            eprintln!("failed to position window with swaymsg: {error:#}");
        }
        return;
    }

    eprintln!(
        "window position `{}` requested, but no supported positioning backend is available",
        ui.position
    );
}

fn position_with_sway(window: &ApplicationWindow, ui: &UiConfig) -> Result<()> {
    let position = WindowPosition::parse(&ui.position).unwrap_or(WindowPosition::TopLeft);
    let window_width = window.allocated_width().max(ui.window_width).max(1);
    let window_height = window.allocated_height().max(1);
    let (x, y) = if position == WindowPosition::Custom {
        (ui.x, ui.y)
    } else {
        calculate_window_position(
            position,
            focused_sway_workspace()?,
            window_width,
            window_height,
            ui.x,
            ui.y,
        )
    };

    let command = format!(
        r#"[app_id="{APP_ID}"] floating enable, resize set width {} px, move absolute position {} px {} px"#,
        ui.window_width, x, y
    );

    let output = Command::new("swaymsg")
        .arg(command)
        .output()
        .context("run swaymsg window command")?;
    if !output.status.success() {
        anyhow::bail!(
            "swaymsg window command exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let results: Vec<SwayCommandResult> =
        serde_json::from_slice(&output.stdout).context("parse swaymsg window result")?;
    if let Some(result) = results.into_iter().find(|result| !result.success) {
        anyhow::bail!(
            "swaymsg rejected window command: {}",
            result.error.as_deref().unwrap_or("unknown error")
        );
    }
    Ok(())
}

fn focused_sway_workspace() -> Result<SwayRect> {
    let output = Command::new("swaymsg")
        .args(["-t", "get_workspaces", "-r"])
        .output()
        .context("query Sway workspaces")?;
    if !output.status.success() {
        anyhow::bail!(
            "swaymsg workspace query exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let workspaces: Vec<SwayWorkspace> =
        serde_json::from_slice(&output.stdout).context("parse Sway workspace geometry")?;
    workspaces
        .into_iter()
        .find(|workspace| workspace.focused)
        .map(|workspace| workspace.rect)
        .context("Sway reported no focused workspace")
}

fn calculate_window_position(
    position: WindowPosition,
    workspace: SwayRect,
    window_width: i32,
    window_height: i32,
    x_offset: i32,
    y_offset: i32,
) -> (i32, i32) {
    if position == WindowPosition::Custom {
        return (x_offset, y_offset);
    }

    let left = workspace.x + x_offset.max(0);
    let center_x = workspace.x + (workspace.width - window_width) / 2 + x_offset;
    let right = workspace.x + workspace.width - window_width - x_offset.max(0);
    let top = workspace.y + y_offset.max(0);
    let center_y = workspace.y + (workspace.height - window_height) / 2 + y_offset;
    let bottom = workspace.y + workspace.height - window_height - y_offset.max(0);

    let (x, y) = match position {
        WindowPosition::TopLeft => (left, top),
        WindowPosition::TopCenter => (center_x, top),
        WindowPosition::TopRight => (right, top),
        WindowPosition::CenterLeft => (left, center_y),
        WindowPosition::Center => (center_x, center_y),
        WindowPosition::CenterRight => (right, center_y),
        WindowPosition::BottomLeft => (left, bottom),
        WindowPosition::BottomCenter => (center_x, bottom),
        WindowPosition::BottomRight => (right, bottom),
        WindowPosition::Custom => unreachable!(),
    };

    let min_x = workspace.x;
    let max_x = workspace.x + (workspace.width - window_width).max(0);
    let min_y = workspace.y;
    let max_y = workspace.y + (workspace.height - window_height).max(0);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path).any(|dir| {
                let path = dir.join(command);
                path.is_file()
                    && fs::metadata(&path)
                        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchored_positions_use_window_geometry_and_offsets() {
        let workspace = SwayRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            calculate_window_position(WindowPosition::TopRight, workspace, 720, 100, 96, 72),
            (1104, 72)
        );
        assert_eq!(
            calculate_window_position(WindowPosition::BottomCenter, workspace, 720, 100, 0, 20),
            (600, 960)
        );
        assert_eq!(
            calculate_window_position(WindowPosition::Custom, workspace, 720, 100, -800, 50),
            (-800, 50)
        );
    }
}
