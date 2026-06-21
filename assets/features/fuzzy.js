// Tiny fzf-style fuzzy matcher. Pure functions, no DOM, no dependencies.
//
// `score(query, candidate)` returns a number (higher is better) when every
// character of `query` appears in `candidate` in order, or `null` otherwise.
// Scoring favours matches at word boundaries (after a path separator, `-`,
// `_`, `.` or space), contiguous runs, and characters in the file's basename
// rather than its directory, so `notes/todo.md` outranks `t/o/d/other.md` for
// the query "todo".

const SEPARATORS = "/\\-_. ";

// Tuned so a boundary or basename match outweighs a stray earlier match, and a
// contiguous run outweighs the same characters scattered across the path.
const MATCH = 1;
const BOUNDARY_BONUS = 10;
const BASENAME_BONUS = 5;
const CONTIGUOUS_BONUS = 8;
const LEADING_PENALTY = 0.5;

export const score = (query, candidate) => {
    if (query === "") {
        return 0;
    }
    const q = query.toLowerCase();
    const c = candidate.toLowerCase();
    const qlen = q.length;
    const clen = c.length;
    if (qlen > clen) {
        return null;
    }

    const lastSlash = c.lastIndexOf("/");

    // Alignment DP. `prevRow[j]` is the best score for matching the first `i`
    // query characters with the i-th matched at candidate position `j`.
    const NEG = -Infinity;
    let prevRow = null;
    for (let i = 0; i < qlen; i++) {
        const row = new Array(clen).fill(NEG);
        // Best `prevRow[0..j-2]` seen so far — the score of extending after a
        // gap (a non-contiguous match), kept one position behind `j`.
        let bestBeforeGap = NEG;
        for (let j = 0; j < clen; j++) {
            if (i > 0 && j >= 2 && prevRow[j - 2] > bestBeforeGap) {
                bestBeforeGap = prevRow[j - 2];
            }
            if (q[i] !== c[j]) {
                continue;
            }
            const cs = charScore(c, j, lastSlash);
            if (i === 0) {
                row[j] = cs - LEADING_PENALTY * j;
                continue;
            }
            const contiguous =
                prevRow[j - 1] > NEG ? prevRow[j - 1] + CONTIGUOUS_BONUS : NEG;
            const base = Math.max(contiguous, bestBeforeGap);
            if (base > NEG) {
                row[j] = base + cs;
            }
        }
        prevRow = row;
    }

    const best = Math.max(...prevRow);
    return best > NEG ? best : null;
};

// Rank `items` by descending score for `query`, dropping non-matches. Items may
// be strings or objects with a `path`; an empty query keeps the input order.
// Returns at most `limit` items (pass `null` for no limit).
export const filter = (query, items, limit = 50) => {
    if (query === "") {
        return limit == null ? items.slice() : items.slice(0, limit);
    }
    const text = (item) => (typeof item === "string" ? item : item.path);
    const scored = [];
    for (const item of items) {
        const s = score(query, text(item));
        if (s !== null) {
            scored.push({ item, score: s });
        }
    }
    scored.sort((a, b) => {
        if (b.score !== a.score) {
            return b.score - a.score;
        }
        const ta = text(a.item);
        const tb = text(b.item);
        return ta.length - tb.length || ta.localeCompare(tb);
    });
    const ranked = scored.map((entry) => entry.item);
    return limit == null ? ranked : ranked.slice(0, limit);
};

const charScore = (candidate, index, lastSlash) => {
    let s = MATCH;
    const atBoundary =
        index === 0 || SEPARATORS.includes(candidate[index - 1]);
    if (atBoundary) {
        s += BOUNDARY_BONUS;
    }
    if (index > lastSlash) {
        s += BASENAME_BONUS;
    }
    return s;
};
