import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useI18n } from "@/i18n";

/** Environment-level banner: the sidecar binary is missing or broken, so no
 * export/check/update can run until fixed. Shared by every view. */
export function SidecarErrorCard({ error }: { error: string }) {
  const { t } = useI18n();
  return (
    <Card className="border-destructive">
      <CardHeader>
        <CardTitle className="text-destructive">
          {t.app.sidecarErrorTitle}
        </CardTitle>
        <CardDescription>
          {t.app.sidecarErrorHint.pre}{" "}
          <code>{t.app.sidecarErrorHint.code}</code>{" "}
          {t.app.sidecarErrorHint.post}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <pre className="max-h-24 overflow-auto rounded-md bg-[var(--background-secondary)] p-2 font-mono text-xs whitespace-pre-wrap">
          {error}
        </pre>
      </CardContent>
    </Card>
  );
}
