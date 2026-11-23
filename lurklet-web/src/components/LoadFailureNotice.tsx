import { CircleAlertIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

export function LoadFailureNotice({
  subject,
  unavailable = false,
  detail,
  onRetry,
  disabled = false,
  retryLabel = "Retry",
}: {
  subject: string;
  unavailable?: boolean;
  detail?: string;
  onRetry?: (() => void) | undefined;
  disabled?: boolean;
  retryLabel?: string;
}) {
  return (
    <Alert>
      <CircleAlertIcon aria-hidden="true" />
      <AlertTitle>
        {subject} {unavailable ? "unavailable" : "failed to load"}
      </AlertTitle>
      <AlertDescription className="select-text">
        {detail}
        {onRetry && (
          <div>
            <Button
              className="mt-2 w-fit"
              size="sm"
              variant="outline"
              isDisabled={disabled}
              onPress={onRetry}
            >
              {retryLabel}
            </Button>
          </div>
        )}
      </AlertDescription>
    </Alert>
  );
}
