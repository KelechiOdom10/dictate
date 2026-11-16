import type { ReactNode } from "react";
import { Card } from "@astryxdesign/core/Card";
import { Heading } from "@astryxdesign/core/Heading";
import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";

export function SettingsRow({
  label,
  help,
  children,
}: {
  label: string;
  help?: ReactNode;
  children: ReactNode;
}) {
  return (
    <ListItem
      label={label}
      description={help ? <Text type="supporting">{help}</Text> : undefined}
      endContent={children}
    />
  );
}
export function SettingsGroup({
  title,
  icon,
  children,
}: {
  title: string;
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <VStack gap={2}>
      <HStack gap={2} vAlign="center">
        {icon}
        <Heading level={2}>{title}</Heading>
      </HStack>
      <Card padding={0}>
        <List hasDividers edge="bleed">
          {children}
        </List>
      </Card>
    </VStack>
  );
}
export function PathText({ children }: { children: string }) {
  return (
    <Text type="code" maxLines={1}>
      {children}
    </Text>
  );
}
