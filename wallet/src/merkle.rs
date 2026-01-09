use poseidon_rs::{Fr, Poseidon};

type Hash = [u8; 32];

#[derive(Debug)]
enum SiblingSide {
    Left,
    Right,
}

#[derive(Debug)]
pub struct Node {
    sibling: Fr,
    sibling_side: SiblingSide,
}
pub struct MerkleTree {
    pub levels: Vec<Vec<Fr>>,
    pub root: Fr,
}

impl MerkleTree {
    pub fn new(leaves: &[Fr]) -> Self {
        let levels = build_tree_levels(leaves);
        let root = *levels.last().unwrap().first().unwrap();
        MerkleTree { levels, root }
    }

    pub fn generate_proof(&self, index: usize) -> Vec<Node> {
        let mut proof: Vec<Node> = vec![];
        let mut current_index = index;

        let levels = &self.levels;

        for level in 0..levels.len() - 1 {
            let siblings_index: usize = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            if siblings_index < levels[level].len() {
                let sibling = levels[level][siblings_index];
                let sibling_side: SiblingSide;
                match current_index % 2 == 0 {
                    true => sibling_side = SiblingSide::Right,
                    false => sibling_side = SiblingSide::Left,
                }
                proof.push(Node {
                    sibling,
                    sibling_side,
                });
            } else {
                proof.push(Node {
                    sibling: levels[level][current_index],
                    sibling_side: SiblingSide::Right,
                })
            }

            current_index = current_index / 2;
        }

        proof
    }

    pub fn verify_proof(&self, leaf_value: &[Fr], proof: &[Node]) -> bool {
        let mut current_hash = hash_leaf(leaf_value.to_vec());

        for node in proof {
            match node.sibling_side {
                SiblingSide::Right => {
                    current_hash = hash_leaf([current_hash, node.sibling].to_vec())
                }
                SiblingSide::Left => {
                    current_hash = hash_leaf([node.sibling, current_hash].to_vec())
                }
            }
        }

        current_hash == self.root
    }
}

fn hash_leaf(input: Vec<Fr>) -> Fr {
    let hasher = Poseidon::new();
    hasher.hash(input).expect("Failed to hash the leaf")
}

fn build_tree_levels(_leaves: &[Fr]) -> Vec<Vec<Fr>> {
    let mut levels = vec![];

    let mut current_tree: Vec<Fr> = _leaves.iter().map(|&c| hash_leaf([c].to_vec())).collect();

    levels.push(current_tree.clone());

    while current_tree.len() > 1 {
        let mut accum = vec![];
        for i in (0..current_tree.len()).step_by(2) {
            if i == current_tree.len() - 1 {
                // Deplicate last node
                let hash = hash_leaf([current_tree[i], current_tree[i]].to_vec());
                accum.push(hash);
                break;
            } else {
                let hash = hash_leaf([current_tree[i], current_tree[i + 1]].to_vec());
                accum.push(hash);
            }
        }
        levels.push(accum.clone());
        current_tree = accum;
    }

    levels
}
