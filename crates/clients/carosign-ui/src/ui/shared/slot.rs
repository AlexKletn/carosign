use obscur_kit::view::view::{SLOT, View, ViewFragment, ViewProperty, project_slots};

pub(in crate::ui) fn fill<const N: usize>(layout: View, slots: [(&str, ViewFragment); N]) -> View {
    let contents = slots
        .into_iter()
        .flat_map(|(slot, nodes)| {
            nodes
                .into_nodes()
                .into_iter()
                .map(move |node| node.with_prop(ViewProperty::new(SLOT, slot.into())))
        })
        .collect();
    project_slots(layout, &ViewFragment::from_nodes(contents))
}
