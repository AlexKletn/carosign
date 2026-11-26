use obscur_kit::runtime::UiRuntime;

pub(super) fn reveal(runtime: &mut UiRuntime, trigger_id: &str) {
    const MENU_CLEARANCE: f32 = 216.0;
    let tree = runtime.inspect_snapshot();
    let trigger = tree
        .nodes
        .iter()
        .find(|node| node.id.as_deref() == Some(trigger_id));
    let content = tree
        .nodes
        .iter()
        .find(|node| node.classes.iter().any(|class| class == "content"));
    let (Some(trigger), Some(content)) = (trigger, content) else {
        return;
    };
    let (Some(trigger_box), Some(content_box)) = (
        runtime.layout_boxes().get(&trigger.node),
        runtime.layout_boxes().get(&content.node),
    ) else {
        return;
    };
    let trigger_bottom = trigger_box.origin().y + trigger_box.size().height;
    let content_bottom = content_box.origin().y + content_box.size().height;
    let missing = trigger_bottom + MENU_CLEARANCE - content_bottom;
    if missing > 0.0 {
        runtime.set_scroll_top(content.node, runtime.scroll_top(content.node) + missing);
    }
}

pub(super) fn focus(runtime: &mut UiRuntime, id: &str) {
    let target = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.id.as_deref() == Some(id))
        .map(|node| node.node);
    if let Some(target) = target {
        runtime.focus_node(Some(target));
    }
}
