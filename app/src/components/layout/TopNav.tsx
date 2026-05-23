import { Box, TabNav } from "@radix-ui/themes";

import { pages, type PageId } from "../../types/ui";

export type TopNavProps = {
  activePage: PageId;
  onNavigate: (page: PageId) => void;
  serverCount: number;
};

export default function TopNav({ activePage, onNavigate, serverCount }: TopNavProps) {
  return (
    <Box asChild>
      <nav aria-label="Primary navigation">
        <TabNav.Root size="2" color="bronze">
          {pages.map((page) => (
            <TabNav.Link
              key={page.id}
              href="#"
              active={page.id === activePage}
              onClick={(event) => {
                event.preventDefault();
                onNavigate(page.id);
              }}
            >
              {page.id === "servers" ? `${page.label} (${serverCount})` : page.label}
            </TabNav.Link>
          ))}
        </TabNav.Root>
      </nav>
    </Box>
  );
}
