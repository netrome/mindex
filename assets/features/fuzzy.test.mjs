// Unit tests for the fuzzy matcher. Run with:  node --test assets/
// Uses only the Node.js built-in test runner and assert module (no third-party
// dependencies). Not part of the Rust build or the shipped binary.

import { test } from "node:test";
import assert from "node:assert/strict";
import { score, filter } from "./fuzzy.js";

const ranks = (query, candidates) => filter(query, candidates, null);

test("score returns null for non-subsequence queries", () => {
    assert.equal(score("xyz", "notes/todo.md"), null);
    assert.equal(score("dot", "todo"), null); // out of order
});

test("score matches a subsequence case-insensitively", () => {
    assert.notEqual(score("TODO", "notes/todo.md"), null);
    assert.notEqual(score("nt", "notes/todo.md"), null);
});

test("empty query scores neutrally and keeps order", () => {
    assert.equal(score("", "anything.md"), 0);
    assert.deepEqual(ranks("", ["b.md", "a.md"]), ["b.md", "a.md"]);
});

test("contiguous basename match outranks scattered path match", () => {
    // Given
    const candidates = ["t/o/d/other.md", "notes/todo.md"];

    // When
    const ranked = ranks("todo", candidates);

    // Then
    assert.equal(ranked[0], "notes/todo.md");
});

test("basename match outranks an incidental directory match", () => {
    // Given
    const candidates = ["todo/archive.md", "projects/todo.md"];

    // When
    const ranked = ranks("todo", candidates);

    // Then
    assert.equal(ranked[0], "projects/todo.md");
});

test("boundary match outranks a mid-word match", () => {
    // Given
    const candidates = ["readme.md", "my-report.md"];

    // When: "rep" starts a word in my-report.md, but is mid-word in readme
    const ranked = ranks("rep", candidates);

    // Then
    assert.equal(ranked[0], "my-report.md");
});

test("filter drops non-matches and respects the limit", () => {
    // Given
    const candidates = ["alpha.md", "beta.md", "gamma.md"];

    // When
    const matches = filter("a", candidates, 2);

    // Then
    assert.equal(matches.length, 2);
    assert.ok(!matches.includes("beta.md"));
});

test("filter works on objects with a path field", () => {
    // Given
    const items = [
        { path: "notes/todo.md", kind: "document" },
        { path: "scan.pdf", kind: "pdf" },
    ];

    // When
    const matches = filter("todo", items, null);

    // Then
    assert.equal(matches.length, 1);
    assert.equal(matches[0].kind, "document");
});

test("ties break toward the shorter path", () => {
    // Given two boundary-starting basename matches for "x"
    const candidates = ["x.md", "xenon-config.md"];

    // When
    const ranked = ranks("x", candidates);

    // Then the shorter, fully-matched name wins
    assert.equal(ranked[0], "x.md");
});
