export type RetainedQueryView<Data, Context> = Data & { __retainedContext?: Context };

// Placeholder-only context keeps actions tied to the displayed result without changing cached API responses.
export function retainQueryView<Data extends object, Context>(
  data: Data,
  context: Context,
): RetainedQueryView<Data, Context> {
  return { ...data, __retainedContext: context };
}
