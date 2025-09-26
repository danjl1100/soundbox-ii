- [x] define requirements by discussing details for the visualizer features, example high level items:
    - display the bucket-spigot network
    - display information about specific nodes, when hovered
    - allow the user to modify the network (edit nodes, remove nodes, add nodes)
- FEEDBACK from initial plan:
    - section 1. Network Display
        - Node Visualization
            - Joints - don't need child count display (should be apparent by the parent-child relationship visuals)
            - Weights - in addition to displaying the weight value, could also have it influence the parent-child relationship visualization
    - sections 2. to 5. are all great details
    - Answers to Questions
        - 1. Focus on read-only to start, with placeholders for editing UI elements (e.g. buttons / textboxes / combos, no drop zones for now)
        - 2. Assume filters are strings for now in the placeholder UI elements, but OK to not hook it up to the TableView yet (I realize now it's not included in the sample TableView JSON - ok to save that as a future enhancement)
        - 3. Bucket item contents - for the UI assume items are strings, but the data model only has a total count. For now, can use placeholders like "Item 1", "Item 2", etc.
        - 4. For the layout, it looks best to center around Buckets equally spaced as rows, with different indentation based on the depth, and then placing the Joints vertically centered around their child nodes.
        - 5. Integration - good questions! This will need to have additional player UI like album art (small), buttons for play/pause, next, previous, and a collapsible list of recently played items. This is just a stretch goal for now, so can put placeholders in the UIfor these elements, or a generic overall playback placeholder. I'm not sure at this point how it would look to "put the spigot visuals in a box" and tack on player UI later. Thoughts?


## wait until requirements are fully agreed upon
- [ ] implement a few different prototype versions of the UI and node rendering. Note that the existing content in app.ts is completely throw-away: only the `vanjs` and general build system should stay unchanged, for lean dependencies (unless there is a strong compelling resion)
