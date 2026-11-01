import type { Literal, Nodes, Root } from "mdast";
import type { Code, State, Tokenizer } from "micromark-util-types";
import type {} from "remark-parse";
import type { Processor } from "unified";

type RedditMention = Literal & { type: "redditMention"; value: string };

declare module "mdast" {
  interface RootContentMap {
    redditMention: RedditMention;
  }
  interface PhrasingContentMap {
    redditMention: RedditMention;
  }
}

declare module "micromark-util-types" {
  interface TokenTypeMap {
    redditMention: "redditMention";
  }
}

export function remarkRedditAutolinks(this: Processor) {
  const data = this.data();
  const mention = { tokenize: tokenizeMention };
  (data.micromarkExtensions ??= []).push({ text: { 47: mention, 114: mention, 117: mention } });
  (data.fromMarkdownExtensions ??= []).push({
    enter: {
      redditMention(token) {
        this.enter({ type: "redditMention", value: this.sliceSerialize(token) }, token);
      },
    },
    exit: {
      redditMention(token) {
        this.exit(token);
      },
    },
  });

  return (tree: Root) => {
    resolveMentions(tree, false);
    return tree;
  };
}

function resolveMentions(node: Nodes, inLink: boolean) {
  if (!("children" in node)) return;
  inLink ||= node.type === "link" || node.type === "linkReference";
  for (let index = 0; index < node.children.length; index++) {
    const child = node.children[index]!;
    if (child.type === "redditMention") {
      const text = {
        type: "text" as const,
        value: child.value,
        ...(child.position && { position: child.position }),
      };
      node.children[index] = inLink
        ? text
        : {
            type: "link",
            url: child.value.startsWith("/") ? child.value : `/${child.value}`,
            children: [text],
            ...(child.position && { position: child.position }),
          };
    } else {
      resolveMentions(child, inLink);
    }
  }
}

const tokenizeMention: Tokenizer = function (effects, ok, nok) {
  let user = false;
  let size = 0;
  const previous = this.previous;
  return (code) => {
    if (
      previous !== null &&
      previous >= 0 &&
      /[\p{L}\p{N}_/@.\\-]/u.test(String.fromCodePoint(previous))
    )
      return nok(code);
    effects.enter("redditMention");
    if (code === 47) {
      effects.consume(code);
      return prefix;
    }
    return prefix(code);
  };

  function prefix(code: Code): State | undefined {
    if (code !== 114 && code !== 117) return nok(code);
    user = code === 117;
    effects.consume(code);
    return (code) => {
      if (code !== 47) return nok(code);
      effects.consume(code);
      return name;
    };
  }

  function name(code: Code): State | undefined {
    if (
      code !== null &&
      code >= 0 &&
      (user ? /[a-z\d_-]/i : /[a-z\d_]/i).test(String.fromCodePoint(code))
    ) {
      size++;
      effects.consume(code);
      return name;
    }
    if (!size || code === 47) return nok(code);
    effects.exit("redditMention");
    return ok(code);
  }
};
