export function NotFoundPage() {
  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-2xl font-semibold">Page not found</h1>
      <p className="text-muted-foreground">This address does not match a supported page.</p>
    </div>
  );
}
