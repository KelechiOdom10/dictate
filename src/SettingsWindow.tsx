import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { flushSync } from "react-dom";
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useQuery } from "@tanstack/react-query";
import General from "./GeneralSettings";
import HistoryPage from "./HistoryPage";
import AnnotatePage from "./AnnotatePage";
import ModelsPage from "./ModelsPage";
import OutputPage from "./OutputPage";
import AdvancedPage from "./AdvancedPage";
import AboutPage from "./AboutPage";
import AppearancePage from "./AppearancePage";
import { Banner } from "@astryxdesign/core/Banner";
import { Link } from "@astryxdesign/core/Link";
import { Heading } from "@astryxdesign/core/Heading";
import { AppShell } from "@astryxdesign/core/AppShell";
import {
  SideNav,
  SideNavHeading,
  SideNavItem,
} from "@astryxdesign/core/SideNav";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";
import {
  AudioLines,
  SlidersHorizontal,
  Cpu,
  History,
  TextCursorInput,
  Settings2,
  Info,
  PenLine,
  SunMoon,
} from "lucide-react";
import { useDictationStatus } from "./useDictationStatus";
import { readSettings } from "./settings";
import { checkForUpdates } from "./update";
import {
  SettingsTheme,
  shownDark,
  themeForMode,
  useSystemDark,
} from "./ui/SettingsTheme";

const subtitles: Record<string, string> = {
  General: "Make dictate feel like second nature.",
  Appearance: "Pick a theme, and whether it follows your Mac’s light and dark.",
  Models:
    "Choose how dictate turns speech into text. Everything runs on this Mac.",
  Annotate:
    "Record your screen, draw on it and talk. Stop to copy it for your agent.",
  History: "Everything you’ve dictated and annotated, kept on this Mac.",
  Output: "Decide what happens to your words once you stop speaking.",
  Advanced: "Fine-tune the microphone, overlay and troubleshooting.",
  About: "Version, source code and where dictate keeps its files.",
};

const pages = [
  { name: "General", icon: SlidersHorizontal },
  { name: "Appearance", icon: SunMoon },
  { name: "Models", icon: Cpu },
  { name: "Annotate", icon: PenLine },
  { name: "History", icon: History },
  { name: "Output", icon: TextCursorInput },
  { name: "Advanced", icon: Settings2 },
  { name: "About", icon: Info },
];

export default function SettingsWindow({
  initialPage = "General",
}: {
  initialPage?: string;
}) {
  const status = useDictationStatus();
  const [page, setPage] = useState(initialPage);
  const [navigationReady, setNavigationReady] = useState(false);
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const dark = shownDark(
    settings.data?.appearanceMode ?? "system",
    useSystemDark(),
  );
  useEffect(() => {
    let disposed = false;
    const subscription = listen("settings-page", ({ payload }) => {
      if (
        typeof payload === "string" &&
        pages.some(({ name }) => name === payload)
      )
        setPage(payload);
    });
    void subscription
      .then(() => {
        if (!disposed) setNavigationReady(true);
      })
      .catch(console.error);
    return () => {
      disposed = true;
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, []);
  useEffect(() => {
    if (!navigationReady || !settings.isFetched) return;
    void invoke("settings_ready").catch(console.error);
  }, [navigationReady, settings.isFetched]);
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  function open(next: string) {
    if (
      document.startViewTransition &&
      !matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      document.startViewTransition(() => flushSync(() => setPage(next)));
    } else setPage(next);
  }
  return (
    <SettingsTheme
      theme={themeForMode(
        {
          themeLight: settings.data?.themeLight ?? "ember",
          themeDark: settings.data?.themeDark ?? "ember",
        },
        dark,
      )}
      // Until the settings arrive the window has no pair of themes, so it
      // follows the Mac; once they do, the resolved side is explicit.
      mode={settings.data ? (dark ? "dark" : "light") : "system"}
    >
      <AppShell
        variant="wash"
        contentPadding={6}
        // The window's own floor is 640, so this only fires in a browser
        // narrower than the app can be; without it the content column is
        // squeezed under the sidebar and a control can outgrow it.
        mobileNav={{ breakpoint: "sm" }}
        sideNav={
          <SideNav
            resizable={{
              defaultWidth: 200,
              minWidth: 180,
              maxWidth: 280,
              autoSaveId: "settings-nav",
            }}
            header={
              <SideNavHeading
                heading="dictate"
                icon={<AudioLines size={20} aria-hidden="true" />}
              />
            }
            footer={
              <VStack gap={1}>
                <Text type="supporting">Version {version.data ?? "…"}</Text>
                <Link
                  as="button"
                  type="supporting"
                  onClick={() => {
                    open("About");
                    void checkForUpdates();
                  }}
                >
                  Check for updates
                </Link>
              </VStack>
            }
          >
            {pages.map(({ name, icon }) => (
              <SideNavItem
                key={name}
                label={name}
                icon={icon}
                isSelected={page === name}
                onClick={() => open(name)}
              />
            ))}
          </SideNav>
        }
      >
        <VStack key={page} gap={6}>
          <VStack gap={1}>
            <Heading level={1}>{page}</Heading>
            <Text color="secondary">{subtitles[page]}</Text>
          </VStack>
          {status.error && (
            <Banner
              role="alert"
              status="error"
              title={status.error.title}
              description={status.error.fix}
              collapsible={false}
              isDismissable
              dismissLabel="Dismiss"
              onDismiss={() => {
                void invoke("dismiss_error").catch(console.error);
              }}
            />
          )}
          {page === "General" ? (
            <General onOpenModels={() => setPage("Models")} />
          ) : page === "Appearance" ? (
            <AppearancePage />
          ) : page === "Models" ? (
            <ModelsPage />
          ) : page === "Annotate" ? (
            <AnnotatePage />
          ) : page === "History" ? (
            <HistoryPage />
          ) : page === "Output" ? (
            <OutputPage />
          ) : page === "Advanced" ? (
            <AdvancedPage />
          ) : (
            <AboutPage />
          )}
        </VStack>
      </AppShell>
    </SettingsTheme>
  );
}
