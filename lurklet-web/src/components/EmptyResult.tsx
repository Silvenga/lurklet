import { Empty, EmptyDescription } from "@/components/ui/empty";

export function EmptyResult({ children }: { children: string }) {
  return (
    <Empty className="items-start p-0 text-start">
      <EmptyDescription>{children}</EmptyDescription>
    </Empty>
  );
}
