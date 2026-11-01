import type { Parent, PhrasingContent } from "mdast";
import { codeText } from "micromark-core-commonmark";
import { markdownLineEnding, markdownLineEndingOrSpace } from "micromark-util-character";
import type { Code, Construct, State, Tokenizer } from "micromark-util-types";
import type {} from "remark-parse";
import type { Processor } from "unified";

type RedditSuperscript = Parent & { type: "redditSuperscript"; children: PhrasingContent[] };

declare module "mdast" {
  interface RootContentMap {
    redditSuperscript: RedditSuperscript;
  }
  interface PhrasingContentMap {
    redditSuperscript: RedditSuperscript;
  }
}

declare module "micromark-util-types" {
  interface TokenTypeMap {
    redditSuperscript: "redditSuperscript";
    redditSuperscriptMarker: "redditSuperscriptMarker";
  }
}

export function remarkRedditSuperscript(this: Processor) {
  const data = this.data();
  (data.micromarkExtensions ??= []).push({ text: { 94: { tokenize: tokenizeSuperscript } } });
  (data.fromMarkdownExtensions ??= []).push({
    canContainEols: ["redditSuperscript"],
    enter: {
      redditSuperscript(token) {
        this.enter({ type: "redditSuperscript", children: [], data: { hName: "sup" } }, token);
      },
    },
    exit: {
      redditSuperscript(token) {
        this.exit(token);
      },
    },
  });
}

const tokenizeSuperscript: Tokenizer = function (effects, ok, nok) {
  const now = this.now.bind(this);
  let grouped = false;
  let depth = 0;
  let contentStart = 0;
  let codeEnd = 0;
  const codeSpan: Construct = {
    partial: true,
    tokenize(effects, ok, nok) {
      return effects.attempt(
        codeText,
        (code) => {
          codeEnd = this.now().offset;
          return ok(code);
        },
        nok,
      );
    },
  };
  const spoilerEnd: Construct = {
    partial: true,
    tokenize(effects, ok, nok) {
      return (code) => {
        effects.enter("redditSuperscriptMarker");
        effects.consume(code);
        return (code) => {
          if (code !== 60) return nok(code);
          effects.consume(code);
          effects.exit("redditSuperscriptMarker");
          return ok;
        };
      };
    },
  };

  return (code) => {
    effects.enter("redditSuperscript");
    effects.enter("redditSuperscriptMarker");
    effects.consume(code);
    return afterCaret;
  };

  function afterCaret(code: Code): State | undefined {
    if (code === 40) {
      grouped = true;
      effects.consume(code);
      return startContent;
    }
    return startContent(code);
  }

  function startContent(code: Code): State | undefined {
    if (code === null || markdownLineEndingOrSpace(code) || code === 41 || code === 94)
      return nok(code);
    effects.exit("redditSuperscriptMarker");
    contentStart = now().offset;
    effects.enter("chunkText", { contentType: "text" });
    return content(code);
  }

  function content(code: Code): State | undefined {
    if (
      code === null ||
      markdownLineEnding(code) ||
      (!grouped &&
        (markdownLineEndingOrSpace(code) ||
          code === 94 ||
          code === 60 ||
          code === 62 ||
          code === 42 ||
          code === 95 ||
          code === 93))
    )
      return end(code);
    if (!grouped && code === 33) return effects.check(spoilerEnd, end, consume)(code);
    if (grouped && code === 41) {
      if (depth === 0) {
        effects.exit("chunkText");
        effects.enter("redditSuperscriptMarker");
        effects.consume(code);
        effects.exit("redditSuperscriptMarker");
        effects.exit("redditSuperscript");
        return ok;
      }
      depth--;
    } else if (grouped && code === 40) {
      depth++;
    }
    if (code === 92) {
      effects.consume(code);
      return escaped;
    }
    if (code === 96) return effects.check(codeSpan, inCode, consume)(code);
    return consume(code);
  }

  function consume(code: Code): State | undefined {
    effects.consume(code);
    return content;
  }

  function end(code: Code): State | undefined {
    if (grouped || now().offset === contentStart) return nok(code);
    effects.exit("chunkText");
    effects.exit("redditSuperscript");
    return ok(code);
  }

  function escaped(code: Code): State | undefined {
    if (code === null || markdownLineEnding(code)) return content(code);
    effects.consume(code);
    return content;
  }

  function inCode(code: Code): State | undefined {
    if (now().offset === codeEnd) return content(code);
    if (code === null || markdownLineEnding(code)) return nok(code);
    effects.consume(code);
    return inCode;
  }
};
