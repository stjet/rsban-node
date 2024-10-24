use num_format::{Locale, ToFormattedString};
use rsnano_node::transport::{FairQueueInfo, QueueInfo};
use std::{fmt::Debug, hash::Hash};
use strum::IntoEnumIterator;

pub(crate) struct QueueGroupViewModel {
    pub heading: String,
    pub queues: Vec<QueueViewModel>,
}

pub(crate) struct QueueViewModel {
    pub label: String,
    pub value: String,
    pub max: String,
    pub progress: f32,
}

pub(crate) fn create_queue_group_view_model<T, H>(
    heading: H,
    info: &FairQueueInfo<T>,
) -> QueueGroupViewModel
where
    T: Clone + Hash + Eq + IntoEnumIterator + Debug,
    H: Into<String>,
{
    let mut queues = Vec::new();
    for source in T::iter() {
        let info = info
            .queues
            .get(&source)
            .cloned()
            .unwrap_or_else(|| QueueInfo {
                source,
                size: 0,
                max_size: 0,
            });

        queues.push(QueueViewModel {
            label: format!("{:?}", info.source),
            value: info.size.to_formatted_string(&Locale::en),
            max: info.max_size.to_formatted_string(&Locale::en),
            progress: info.size as f32 / info.max_size as f32,
        });
    }

    queues.push(QueueViewModel {
        label: "Total".to_string(),
        value: info.total_size.to_formatted_string(&Locale::en),
        max: info.total_max_size.to_formatted_string(&Locale::en),
        progress: info.total_size as f32 / info.total_max_size as f32,
    });

    QueueGroupViewModel {
        heading: heading.into(),
        queues,
    }
}
