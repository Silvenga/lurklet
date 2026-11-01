import type { Parent, PhrasingContent, Root } from "mdast";
import { blockQuote } from "micromark-core-commonmark";
import { factorySpace } from "micromark-factory-space";
import { markdownSpace } from "micromark-util-character";
import { splice } from "micromark-util-chunked";
import { resolveAll } from "micromark-util-resolve-all";
import type { Construct, Event, Resolver, Token, Tokenizer } from "micromark-util-types";
import type {} from "remark-parse";
import type { Processor } from "unified";
import { SKIP, visit } from "unist-util-visit";

type RedditSpoiler = Parent & { type: "redditSpoiler"; children: PhrasingContent[] };
const SPOILER_DATA = { hName: "span", hProperties: { "data-reddit-spoiler": true } };

declare module "mdast" {
  interface RootContentMap {
    redditSpoiler: RedditSpoiler;
  }
  interface PhrasingContentMap {
    redditSpoiler: RedditSpoiler;
  }
}

declare module "micromark-util-types" {
  interface TokenTypeMap {
    redditSpoiler: "redditSpoiler";
    redditSpoilerMarker: "redditSpoilerMarker";
    redditSpoilerSequence: "redditSpoilerSequence";
  }
}

export function remarkRedditSpoilers(this: Processor) {
  const data = this.data();
  const delimiter: Construct = { tokenize: tokenizeDelimiter, resolveAll: resolveSpoilers };
  (data.micromarkExtensions ??= []).push({
    disable: { null: ["blockQuote"] },
    document: { 62: redditBlockQuote },
    text: { 62: delimiter, 33: delimiter },
    insideSpan: { null: [delimiter] },
  });
  (data.fromMarkdownExtensions ??= []).push({
    canContainEols: ["redditSpoiler"],
    enter: {
      redditSpoiler(token) {
        this.enter(
          {
            type: "redditSpoiler",
            children: [],
            data: SPOILER_DATA,
          },
          token,
        );
      },
    },
    exit: {
      redditSpoiler(token) {
        this.exit(token);
      },
    },
  });
  return (tree: Root) => {
    // Conceal the whole link when its label contains a spoiler, keeping reveal buttons outside anchors.
    visit(tree, (node, index, parent) => {
      if ((node.type !== "link" && node.type !== "linkReference") || index === undefined || !parent)
        return;
      let hasSpoiler = false;
      visit(node, "redditSpoiler", (spoiler, index, parent) => {
        if (index === undefined || !parent) return;
        hasSpoiler = true;
        parent.children.splice(index, 1, ...spoiler.children);
        return index;
      });
      if (hasSpoiler) {
        parent.children[index] = {
          type: "redditSpoiler",
          children: [node],
          data: SPOILER_DATA,
          ...(node.position && { position: node.position }),
        };
        return SKIP;
      }
      return;
    });
    return tree;
  };
}

const tokenizeDelimiter: Tokenizer = function (effects, ok, nok) {
  let opening = false;
  return (code) => {
    opening = code === 62;
    effects.enter("redditSpoilerMarker", { _open: opening, _close: !opening });
    effects.consume(code);
    return (code) => {
      if (code !== (opening ? 33 : 60)) return nok(code);
      effects.consume(code);
      effects.exit("redditSpoilerMarker");
      return ok;
    };
  };
};

const resolveSpoilers: Resolver = (events, context) => {
  for (let close = 0; close < events.length; close++) {
    const closing = events[close]!;
    if (closing[0] !== "enter" || closing[1].type !== "redditSpoilerMarker" || !closing[1]._close)
      continue;
    for (let open = close - 1; open >= 0; open--) {
      const opening = events[open]!;
      if (opening[0] !== "exit" || opening[1].type !== "redditSpoilerMarker" || !opening[1]._open)
        continue;
      if (opening[1].end.offset === closing[1].start.offset) break;
      const token: Token = { type: "redditSpoiler", start: opening[1].start, end: closing[1].end };
      opening[1].type = "redditSpoilerSequence";
      closing[1].type = "redditSpoilerSequence";
      // Keep delimiter events so micromark can still split multiline chunks at their original boundaries.
      const replacement: Event[] = [
        ["enter", token, context],
        ["enter", opening[1], context],
        ["exit", opening[1], context],
      ];
      splice(
        replacement,
        replacement.length,
        0,
        resolveAll(
          context.parser.constructs.insideSpan.null ?? [],
          events.slice(open + 1, close),
          context,
        ),
      );
      replacement.push(
        ["enter", closing[1], context],
        ["exit", closing[1], context],
        ["exit", token, context],
      );
      splice(events, open - 1, close - open + 3, replacement);
      close = open + replacement.length - 2;
      break;
    }
  }
  for (const event of events) {
    if (event[1].type === "redditSpoilerMarker") event[1].type = "data";
  }
  return events;
};

const spoilerOpening: Construct = {
  partial: true,
  tokenize(effects, ok, nok) {
    return (code) => {
      if (code !== 62) return nok(code);
      effects.enter("redditSpoilerMarker");
      effects.consume(code);
      return (code) => {
        if (code !== 33) return nok(code);
        effects.consume(code);
        effects.exit("redditSpoilerMarker");
        return ok;
      };
    };
  },
};

// Reserve >! for inline spoilers before CommonMark consumes > as a quote marker.
const redditBlockQuote: Construct = {
  ...blockQuote,
  name: "redditBlockQuote",
  tokenize(effects, ok, nok) {
    return effects.check(spoilerOpening, nok, blockQuote.tokenize.call(this, effects, ok, nok));
  },
  continuation: {
    tokenize(effects, ok, nok) {
      const next = effects.attempt(redditBlockQuote, ok, nok);
      return (code) =>
        markdownSpace(code) ? factorySpace(effects, next, "linePrefix", 4)(code) : next(code);
    },
  },
};
