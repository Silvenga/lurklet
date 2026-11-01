import { headingAtx } from "micromark-core-commonmark";
import { factorySpace } from "micromark-factory-space";
import {
  markdownLineEnding,
  markdownLineEndingOrSpace,
  markdownSpace,
} from "micromark-util-character";
import type { Code, State, Tokenizer } from "micromark-util-types";
import type {} from "remark-parse";
import type { Processor } from "unified";

export function remarkRedditHeadings(this: Processor) {
  const data = this.data();
  (data.micromarkExtensions ??= []).push({
    flow: { 35: { ...headingAtx, tokenize: tokenizeHeading } },
  });
}

const tokenizeHeading: Tokenizer = function (effects, ok, nok) {
  let size = 0;
  return (code) => {
    effects.enter("atxHeading");
    effects.enter("atxHeadingSequence");
    return opening(code);
  };

  function opening(code: Code): State | undefined {
    if (code === 35) {
      if (++size > 6) return nok(code);
      effects.consume(code);
      return opening;
    }
    effects.exit("atxHeadingSequence");
    return boundary(code);
  }

  function boundary(code: Code): State | undefined {
    if (code === null || markdownLineEnding(code)) {
      effects.exit("atxHeading");
      return ok(code);
    }
    if (markdownSpace(code)) return factorySpace(effects, boundary, "whitespace")(code);
    if (code === 35) {
      effects.enter("atxHeadingSequence");
      return closing(code);
    }
    effects.enter("atxHeadingText");
    return text(code);
  }

  function closing(code: Code): State | undefined {
    if (code === 35) {
      effects.consume(code);
      return closing;
    }
    effects.exit("atxHeadingSequence");
    return boundary(code);
  }

  function text(code: Code): State | undefined {
    if (code === null || code === 35 || markdownLineEndingOrSpace(code)) {
      effects.exit("atxHeadingText");
      return boundary(code);
    }
    effects.consume(code);
    return text;
  }
};
