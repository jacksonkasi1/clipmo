//! Preserve selection order while giving every image its own native paste.

use crate::models::{ClipItem, ItemKind, PasteFlavor};

pub fn paste_batches(items: &[ClipItem], flavor: PasteFlavor) -> Vec<&[ClipItem]> {
    if items.is_empty() {
        return Vec::new();
    }
    if flavor == PasteFlavor::PlainText || !items.iter().any(|item| item.kind == ItemKind::Image) {
        return vec![items];
    }

    let mut batches = Vec::new();
    let mut start = 0;
    while start < items.len() {
        let mut end = start + 1;
        if items[start].kind != ItemKind::Image {
            let files = items[start].kind == ItemKind::Files;
            while end < items.len()
                && items[end].kind != ItemKind::Image
                && (items[end].kind == ItemKind::Files) == files
            {
                end += 1;
            }
        }
        batches.push(&items[start..end]);
        start = end;
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, kind: ItemKind) -> ClipItem {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": kind, "preview": "", "content": "",
            "hasHtml": false, "hasRtf": false, "image": null, "files": [], "fileAssets": [],
            "sizeBytes": 0, "tags": [], "source": null, "favorite": false,
            "copyCount": 1, "device": {"id": "local", "name": "local",
            "platform": "windows", "color": ""}, "syncStatus": "local",
            "firstCopiedAt": 0, "lastCopiedAt": 0
        }))
        .unwrap()
    }

    fn batch_ids(items: &[ClipItem], flavor: PasteFlavor) -> Vec<Vec<i64>> {
        paste_batches(items, flavor)
            .iter()
            .map(|batch| batch.iter().map(|item| item.id).collect())
            .collect()
    }

    #[test]
    fn images_are_pasted_individually_in_selection_order() {
        let items = [
            item(3, ItemKind::Image),
            item(1, ItemKind::Image),
            item(2, ItemKind::Image),
        ];
        assert_eq!(
            batch_ids(&items, PasteFlavor::Original),
            vec![vec![3], vec![1], vec![2]]
        );
    }

    #[test]
    fn mixed_selection_keeps_text_groups_and_files_around_images() {
        let items = [
            item(1, ItemKind::Text),
            item(2, ItemKind::Link),
            item(3, ItemKind::Image),
            item(4, ItemKind::Files),
            item(5, ItemKind::Text),
        ];
        assert_eq!(
            batch_ids(&items, PasteFlavor::Original),
            vec![vec![1, 2], vec![3], vec![4], vec![5]]
        );
    }

    #[test]
    fn plain_text_and_text_only_selections_still_use_one_paste() {
        let items = [item(1, ItemKind::Image), item(2, ItemKind::Image)];
        assert_eq!(batch_ids(&items, PasteFlavor::PlainText), vec![vec![1, 2]]);
        let items = [item(1, ItemKind::Text), item(2, ItemKind::Link)];
        assert_eq!(batch_ids(&items, PasteFlavor::Original), vec![vec![1, 2]]);
        assert!(paste_batches(&[], PasteFlavor::Original).is_empty());
    }
}
