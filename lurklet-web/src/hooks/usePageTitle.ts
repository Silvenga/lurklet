import { useEffect } from "react";

export function usePageTitle(title: string) {
  useEffect(() => {
    document.title = `${title} - Lurklet`;

    return () => {
      document.title = "Lurklet";
    };
  }, [title]);
}
