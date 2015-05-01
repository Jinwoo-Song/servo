/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::bindings::callback::ExceptionHandling::Rethrow;
use dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use dom::bindings::codegen::Bindings::NodeFilterBinding::NodeFilter;
use dom::bindings::codegen::Bindings::NodeFilterBinding::NodeFilterConstants;
use dom::bindings::error::Fallible;
use dom::bindings::js::{JS, JSRef, MutHeap, OptionalRootable, Rootable};
use dom::bindings::js::Temporary;
use dom::node::{Node, NodeHelpers};

#[jstraceable]
pub enum Filter {
    None,
    Native(fn (node: JSRef<Node>) -> u16),
    JS(NodeFilter)
}

pub trait TraversalHelpers {
    fn get_root_node(&self) -> JS<Node>;
    fn get_current_node(&self) -> MutHeap<JS<Node>>;
    fn get_what_to_show(&self) -> u32;
    fn get_filter(&self) -> Filter;

    // https://dom.spec.whatwg.org/#concept-traverse-children
    fn traverse_children<F, G>(&self,
                               next_child: F,
                               next_sibling: G)
                               -> Fallible<Option<Temporary<Node>>>
        where F: Fn(JSRef<Node>) -> Option<Temporary<Node>>,
              G: Fn(JSRef<Node>) -> Option<Temporary<Node>>
    {
        // "To **traverse children** of type *type*, run these steps:"
        // "1. Let node be the value of the currentNode attribute."
        // "2. Set node to node's first child if type is first, and node's last child if type is last."
        let cur = self.get_current_node().get().root();
        let mut node_op: Option<JSRef<Node>> = next_child(cur.r()).map(|node| node.root().get_unsound_ref_forever());

        // 3. Main: While node is not null, run these substeps:
        'main: loop {
            match node_op {
                None => break,
                Some(node) => {
                    // "1. Filter node and let result be the return value."
                    let result = try!(self.accept_node(node));
                    match result {
                        // "2. If result is FILTER_ACCEPT, then set the currentNode
                        //     attribute to node and return node."
                        NodeFilterConstants::FILTER_ACCEPT => {
                            self.get_current_node().set(JS::from_rooted(node));
                            return Ok(Some(Temporary::from_rooted(node)))
                        },
                        // "3. If result is FILTER_SKIP, run these subsubsteps:"
                        NodeFilterConstants::FILTER_SKIP => {
                            // "1. Let child be node's first child if type is first,
                            //     and node's last child if type is last."
                            match next_child(node) {
                                // "2. If child is not null, set node to child and goto Main."
                                Some(child) => {
                                    node_op = Some(child.root().get_unsound_ref_forever());
                                    continue 'main
                                },
                                None => {}
                            }
                        },
                        _ => {}
                    }
                    // "4. While node is not null, run these substeps:"
                    loop {
                        match node_op {
                            None => break,
                            Some(node) => {
                                // "1. Let sibling be node's next sibling if type is next,
                                //     and node's previous sibling if type is previous."
                                match next_sibling(node) {
                                    // "2. If sibling is not null,
                                    //     set node to sibling and goto Main."
                                    Some(sibling) => {
                                        node_op = Some(sibling.root().get_unsound_ref_forever());
                                        continue 'main
                                    },
                                    None => {
                                        // "3. Let parent be node's parent."
                                        match node.parent_node().map(|p| p.root().get_unsound_ref_forever()) {
                                            // "4. If parent is null, parent is root,
                                            //     or parent is currentNode attribute's value,
                                            //     return null."
                                            None => return Ok(None),
                                            Some(parent) if self.is_root_node(parent)
                                                            || self.is_current_node(parent) =>
                                                             return Ok(None),
                                            // "5. Otherwise, set node to parent."
                                            Some(parent) => node_op = Some(parent)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // "4. Return null."
        Ok(None)
    }

    /*

    // https://dom.spec.whatwg.org/#concept-traverse-siblings
    fn traverse_siblings<F, G>(self,
                               next_child: F,
                               next_sibling: G)
                               -> Fallible<Option<Temporary<Node>>>
        where F: Fn(JSRef<Node>) -> Option<Temporary<Node>>,
              G: Fn(JSRef<Node>) -> Option<Temporary<Node>>
    {
        // "To **traverse siblings** of type *type* run these steps:"
        // "1. Let node be the value of the currentNode attribute."
        let mut node = self.current_node.get().root().get_unsound_ref_forever();
        // "2. If node is root, return null."
        if self.is_root_node(node) {
            return Ok(None)
        }
        // "3. Run these substeps:"
        loop {
            // "1. Let sibling be node's next sibling if type is next,
            //  and node's previous sibling if type is previous."
            let mut sibling_op = next_sibling(node);
            // "2. While sibling is not null, run these subsubsteps:"
            while sibling_op.is_some() {
                // "1. Set node to sibling."
                node = sibling_op.unwrap().root().get_unsound_ref_forever();
                // "2. Filter node and let result be the return value."
                let result = try!(self.accept_node(node));
                // "3. If result is FILTER_ACCEPT, then set the currentNode
                //     attribute to node and return node."
                match result {
                    NodeFilterConstants::FILTER_ACCEPT => {
                        self.current_node.set(JS::from_rooted(node));
                        return Ok(Some(Temporary::from_rooted(node)))
                    },
                    _ => {}
                }
                // "4. Set sibling to node's first child if type is next,
                //     and node's last child if type is previous."
                sibling_op = next_child(node);
                // "5. If result is FILTER_REJECT or sibling is null,
                //     then set sibling to node's next sibling if type is next,
                //     and node's previous sibling if type is previous."
                match (result, &sibling_op) {
                    (NodeFilterConstants::FILTER_REJECT, _)
                    | (_, &None) => sibling_op = next_sibling(node),
                    _ => {}
                }
            }
            // "3. Set node to its parent."
            match node.parent_node().map(|p| p.root().get_unsound_ref_forever()) {
                // "4. If node is null or is root, return null."
                None => return Ok(None),
                Some(n) if self.is_root_node(n) => return Ok(None),
                // "5. Filter node and if the return value is FILTER_ACCEPT, then return null."
                Some(n) => {
                    node = n;
                    match try!(self.accept_node(node)) {
                        NodeFilterConstants::FILTER_ACCEPT => return Ok(None),
                        _ => {}
                    }
                }
            }
            // "6. Run these substeps again."
        }
    }

    // https://dom.spec.whatwg.org/#concept-tree-following
    fn first_following_node_not_following_root(self, node: JSRef<Node>)
                                               -> Option<Temporary<Node>> {
        // "An object A is following an object B if A and B are in the same tree
        //  and A comes after B in tree order."
        match node.next_sibling() {
            None => {
                let mut candidate = node;
                while !self.is_root_node(candidate) && candidate.next_sibling().is_none() {
                    match candidate.parent_node() {
                        None =>
                            // This can happen if the user set the current node to somewhere
                            // outside of the tree rooted at the original root.
                            return None,
                        Some(n) => candidate = n.root().get_unsound_ref_forever()
                    }
                }
                if self.is_root_node(candidate) {
                    None
                } else {
                    candidate.next_sibling()
                }
            },
            it => it
        }
    }

    */

    fn accept_node(&self, node: JSRef<Node>) -> Fallible<u16>;
    /*
    // https://dom.spec.whatwg.org/#concept-node-filter
    fn accept_node(&self, node: JSRef<Node>) -> Fallible<u16> {
        // "To filter node run these steps:"
        // "1. Let n be node's nodeType attribute value minus 1."
        let n = node.NodeType() - 1;
        // "2. If the nth bit (where 0 is the least significant bit) of whatToShow is not set,
        //     return FILTER_SKIP."
        if (self.get_what_to_show() & (1 << n)) == 0 {
            return Ok(NodeFilterConstants::FILTER_SKIP)
        }
        // "3. If filter is null, return FILTER_ACCEPT."
        // "4. Let result be the return value of invoking filter."
        // "5. If an exception was thrown, re-throw the exception."
        // "6. Return result."
        match self.get_filter() {
            Filter::None => Ok(NodeFilterConstants::FILTER_ACCEPT),
            Filter::Native(f) => Ok((f)(node)),
            Filter::JS(callback) => callback.AcceptNode_(self, node, Rethrow)
        }
    }
    */

    fn is_root_node(&self, node: JSRef<Node>) -> bool {
        JS::from_rooted(node) == self.get_root_node()
    }

    fn is_current_node(&self, node: JSRef<Node>) -> bool {
        JS::from_rooted(node) == self.get_current_node().get()
    }
}

