//! Unit tests for RouteParams merging
//!
//! Tests for src/params.rs - verifying parent+child merge,
//! collision handling, and empty params as specified in T012.

#[cfg(test)]
mod params_tests {
    use gpui_navigator::RouteParams;

    #[test]
    fn test_parent_child_merge() {
        // T012 - Test merging parent params with child params
        // Example: parent {workspaceId: "1"} + child {projectId: "2"} = {workspaceId: "1", projectId: "2"}
        let mut parent = RouteParams::new();
        parent.set("workspaceId".to_string(), "123".to_string());
        parent.set("view".to_string(), "list".to_string());

        let mut child = RouteParams::new();
        child.set("projectId".to_string(), "456".to_string());

        let merged = RouteParams::merge(&parent, &child);

        assert_eq!(merged.get("workspaceId"), Some(&"123".to_string()));
        assert_eq!(merged.get("projectId"), Some(&"456".to_string()));
        assert_eq!(merged.get("view"), Some(&"list".to_string()));
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_collision_child_wins() {
        // T012 - Test child param overrides parent on collision
        // Example: parent {id: "old"} + child {id: "new"} = {id: "new"}
        let mut parent = RouteParams::new();
        parent.set("id".to_string(), "parent-value".to_string());
        parent.set("type".to_string(), "document".to_string());

        let mut child = RouteParams::new();
        child.set("id".to_string(), "child-value".to_string());

        let merged = RouteParams::merge(&parent, &child);

        // Child wins collision
        assert_eq!(merged.get("id"), Some(&"child-value".to_string()));
        // Parent param without collision preserved
        assert_eq!(merged.get("type"), Some(&"document".to_string()));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_empty_parent() {
        // T012 - Test merging with empty parent RouteParams
        // Example: parent {} + child {id: "1"} = {id: "1"}
        let parent = RouteParams::new();

        let mut child = RouteParams::new();
        child.set("id".to_string(), "123".to_string());
        child.set("name".to_string(), "test".to_string());

        let merged = RouteParams::merge(&parent, &child);

        assert_eq!(merged.get("id"), Some(&"123".to_string()));
        assert_eq!(merged.get("name"), Some(&"test".to_string()));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_empty_child() {
        // T012 - Test merging with empty child RouteParams
        // Example: parent {id: "1"} + child {} = {id: "1"}
        let mut parent = RouteParams::new();
        parent.set("id".to_string(), "123".to_string());
        parent.set("type".to_string(), "document".to_string());

        let child = RouteParams::new();

        let merged = RouteParams::merge(&parent, &child);

        assert_eq!(merged.get("id"), Some(&"123".to_string()));
        assert_eq!(merged.get("type"), Some(&"document".to_string()));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_both_empty() {
        // T012 - Test merging two empty RouteParams
        let parent = RouteParams::new();
        let child = RouteParams::new();

        let merged = RouteParams::merge(&parent, &child);

        assert!(merged.is_empty());
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_multiple_collisions() {
        // T012 - Test multiple param collisions, child always wins
        let mut parent = RouteParams::new();
        parent.set("a".to_string(), "parent-a".to_string());
        parent.set("b".to_string(), "parent-b".to_string());
        parent.set("c".to_string(), "parent-c".to_string());

        let mut child = RouteParams::new();
        child.set("a".to_string(), "child-a".to_string());
        child.set("b".to_string(), "child-b".to_string());
        child.set("d".to_string(), "child-d".to_string());

        let merged = RouteParams::merge(&parent, &child);

        // Collisions: child wins
        assert_eq!(merged.get("a"), Some(&"child-a".to_string()));
        assert_eq!(merged.get("b"), Some(&"child-b".to_string()));
        // No collision: parent value preserved
        assert_eq!(merged.get("c"), Some(&"parent-c".to_string()));
        // Child-only param
        assert_eq!(merged.get("d"), Some(&"child-d".to_string()));
        assert_eq!(merged.len(), 4);
    }

    #[test]
    fn test_deep_nesting_simulation() {
        // T012 - Simulate 3-level param inheritance
        // Level 1: workspaceId
        let mut level1 = RouteParams::new();
        level1.set("workspaceId".to_string(), "ws-123".to_string());

        // Level 2: projectId
        let mut level2 = RouteParams::new();
        level2.set("projectId".to_string(), "proj-456".to_string());
        let merged_l2 = RouteParams::merge(&level1, &level2);

        // Level 3: taskId
        let mut level3 = RouteParams::new();
        level3.set("taskId".to_string(), "task-789".to_string());
        let merged_l3 = RouteParams::merge(&merged_l2, &level3);

        // Final should have all params from all levels
        assert_eq!(merged_l3.get("workspaceId"), Some(&"ws-123".to_string()));
        assert_eq!(merged_l3.get("projectId"), Some(&"proj-456".to_string()));
        assert_eq!(merged_l3.get("taskId"), Some(&"task-789".to_string()));
        assert_eq!(merged_l3.len(), 3);
    }

    // T049: Additional tests for User Story 5 - Parameter Inheritance

    #[test]
    fn test_from_path_single_param() {
        // T049 - Extract single parameter from path
        let params = RouteParams::from_path("/users/123", "/users/:id");

        assert_eq!(params.get("id"), Some(&"123".to_string()));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_from_path_multiple_params() {
        // T049 - Extract multiple parameters from path
        let params = RouteParams::from_path("/users/123/posts/456", "/users/:userId/posts/:postId");

        assert_eq!(params.get("userId"), Some(&"123".to_string()));
        assert_eq!(params.get("postId"), Some(&"456".to_string()));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_from_path_no_match() {
        // T049 - Different number of segments returns empty
        let params = RouteParams::from_path("/users/123/extra", "/users/:id");

        assert!(params.is_empty());
    }

    #[test]
    fn test_from_path_static_mismatch() {
        // T049 - Static segment mismatch returns empty
        let params = RouteParams::from_path("/products/123", "/users/:id");

        assert!(params.is_empty());
    }

    #[test]
    fn test_from_path_with_type_constraints() {
        // T049 - Handle type constraints like :id<i32>
        let params = RouteParams::from_path("/users/123", "/users/:id<i32>");

        assert_eq!(params.get("id"), Some(&"123".to_string()));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_four_level_param_merging() {
        // T049 - Test 4-level parameter inheritance (workspace → project → task → subtask)
        let mut workspace = RouteParams::new();
        workspace.set("workspaceId".to_string(), "ws-1".to_string());

        let mut project = RouteParams::new();
        project.set("projectId".to_string(), "proj-2".to_string());
        let merged_proj = RouteParams::merge(&workspace, &project);

        let mut task = RouteParams::new();
        task.set("taskId".to_string(), "task-3".to_string());
        let merged_task = RouteParams::merge(&merged_proj, &task);

        let mut subtask = RouteParams::new();
        subtask.set("subtaskId".to_string(), "sub-4".to_string());
        let merged_final = RouteParams::merge(&merged_task, &subtask);

        // All 4 levels should be present
        assert_eq!(merged_final.get("workspaceId"), Some(&"ws-1".to_string()));
        assert_eq!(merged_final.get("projectId"), Some(&"proj-2".to_string()));
        assert_eq!(merged_final.get("taskId"), Some(&"task-3".to_string()));
        assert_eq!(merged_final.get("subtaskId"), Some(&"sub-4".to_string()));
        assert_eq!(merged_final.len(), 4);
    }

    #[test]
    fn test_param_collision_in_deep_hierarchy() {
        // T049 - Child overrides parent in multi-level hierarchy
        let mut level1 = RouteParams::new();
        level1.set("id".to_string(), "level1-id".to_string());
        level1.set("status".to_string(), "active".to_string());

        let mut level2 = RouteParams::new();
        level2.set("id".to_string(), "level2-id".to_string()); // Override
        let merged = RouteParams::merge(&level1, &level2);

        // level2 "id" should win
        assert_eq!(merged.get("id"), Some(&"level2-id".to_string()));
        // level1 "status" should be preserved
        assert_eq!(merged.get("status"), Some(&"active".to_string()));
    }

    #[test]
    fn test_settings_page_scenario() {
        // T049 - Simulate the T048 example: /workspace/:workspaceId/project/:projectId/settings
        // Settings page receives both workspaceId and projectId

        // Simulate workspace level
        let mut workspace_params = RouteParams::new();
        workspace_params.set("workspaceId".to_string(), "123".to_string());

        // Simulate project level
        let mut project_params = RouteParams::new();
        project_params.set("projectId".to_string(), "456".to_string());
        let merged_project = RouteParams::merge(&workspace_params, &project_params);

        // Simulate settings level (no new params, but inherits both)
        let settings_params = RouteParams::new();
        let final_params = RouteParams::merge(&merged_project, &settings_params);

        // Settings page should have access to both params
        assert_eq!(final_params.get("workspaceId"), Some(&"123".to_string()));
        assert_eq!(final_params.get("projectId"), Some(&"456".to_string()));
        assert_eq!(final_params.len(), 2);
    }
}
