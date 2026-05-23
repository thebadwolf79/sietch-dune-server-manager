import { Box, Heading, Text } from "@radix-ui/themes";

export type EmptyStateProps = {
  title: string;
  body: string;
};

export default function EmptyState({ title, body }: EmptyStateProps) {
  return (
    <Box className="empty-state">
      <Heading size="4">{title}</Heading>
      <Text as="p" size="2" color="gray">
        {body}
      </Text>
    </Box>
  );
}
