import { useSearchParams } from "react-router";
import type { ListSubredditPostsData } from "@/clients/lurklet-api/generated/types.gen";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

type PostListControlsProps = Pick<
  NonNullable<ListSubredditPostsData["query"]>,
  "sort" | "timeRange"
>;

export function PostListControls({ sort, timeRange }: PostListControlsProps) {
  const [searchParams, setSearchParams] = useSearchParams();

  return (
    <div
      className="flex max-w-full scroll-p-1 items-center gap-3 overflow-x-auto p-1"
      onFocusCapture={(event) => {
        event.target.scrollIntoView({ block: "nearest", inline: "nearest" });
      }}
    >
      <ToggleGroup
        aria-label="Sort"
        className="shrink-0"
        variant="outline"
        spacing={0}
        selectionMode="single"
        disallowEmptySelection
        selectedKeys={[sort ?? "hot"]}
        onSelectionChange={(keys) => {
          const nextSort = String([...keys][0]);
          if (nextSort === sort) {
            return;
          }
          const next = new URLSearchParams(searchParams);
          next.set("sort", nextSort);
          next.delete("after");
          if (nextSort !== "top") {
            next.delete("t");
          }
          void setSearchParams(next);
        }}
      >
        <ToggleGroupItem id="hot">Hot</ToggleGroupItem>
        <ToggleGroupItem id="new">New</ToggleGroupItem>
        <ToggleGroupItem id="top">Top</ToggleGroupItem>
      </ToggleGroup>
      {sort === "top" && (
        <ToggleGroup
          aria-label="Time range"
          className="shrink-0"
          variant="outline"
          spacing={0}
          selectionMode="single"
          disallowEmptySelection
          selectedKeys={[timeRange ?? "all"]}
          onSelectionChange={(keys) => {
            const nextTimeRange = String([...keys][0]);
            if (nextTimeRange === timeRange) {
              return;
            }
            const next = new URLSearchParams(searchParams);
            next.set("t", nextTimeRange);
            next.delete("after");
            void setSearchParams(next);
          }}
        >
          <ToggleGroupItem id="hour">Hour</ToggleGroupItem>
          <ToggleGroupItem id="day">Day</ToggleGroupItem>
          <ToggleGroupItem id="week">Week</ToggleGroupItem>
          <ToggleGroupItem id="month">Month</ToggleGroupItem>
          <ToggleGroupItem id="year">Year</ToggleGroupItem>
          <ToggleGroupItem id="all">All time</ToggleGroupItem>
        </ToggleGroup>
      )}
    </div>
  );
}
