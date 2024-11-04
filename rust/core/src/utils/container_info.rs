use serde_json::json;

#[derive(Clone)]
pub struct ContainerInfo {
    pub name: String,
    pub count: usize,
    pub sizeof_element: usize,
}

#[derive(Clone)]
pub enum ContainerInfoComponent {
    Leaf(ContainerInfo),
    Composite(String, Vec<ContainerInfoComponent>),
}

impl ContainerInfoComponent {
    pub fn into_json(&self) -> serde_json::Value {
        let (key, value) = self.into_json_impl();
        let mut data = serde_json::Map::new();
        data.insert(key, value);
        serde_json::Value::Object(data)
    }

    fn into_json_impl(&self) -> (String, serde_json::Value) {
        match self {
            ContainerInfoComponent::Leaf(leaf) => {
                let fields = json!({"count": leaf.count, "size": leaf.sizeof_element});
                (leaf.name.clone(), fields)
            }
            ContainerInfoComponent::Composite(name, leafs) => {
                let mut leaf_trees = serde_json::Map::new();
                for leaf in leafs {
                    let (name, fields) = leaf.into_json_impl();
                    leaf_trees.insert(name, fields);
                }

                (name.clone(), serde_json::Value::Object(leaf_trees))
            }
        }
    }
}
