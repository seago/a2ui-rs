/**
 * shadcn/ui ComponentKit.
 *
 * Implements the `ComponentKit` contract from `@/contracts` using shadcn/ui
 * components (see `@/components/ui/*`), mapping each A2UI component type to its
 * shadcn rendering. The render core (track V) looks a component up by name and
 * feeds it the normalized props defined by the A2UI protocol; swapping UI
 * libraries only means swapping the kit.
 *
 * @example
 * ```tsx
 * import { shadcnKit } from "@/kits/shadcn";
 *
 * const { Button } = shadcnKit;
 * <Button label="Save" variant="primary" disabled={false} onAction={save} />;
 * ```
 */
import type { ComponentKit } from "@/contracts";

import { AudioPlayer } from "./AudioPlayer";
import { Button } from "./Button";
import { Card } from "./Card";
import { CheckBox } from "./CheckBox";
import { ChoicePicker } from "./ChoicePicker";
import { Column } from "./Column";
import { DateTimeInput } from "./DateTimeInput";
import { Divider } from "./Divider";
import { Icon } from "./Icon";
import { Image } from "./Image";
import { List } from "./List";
import { Modal } from "./Modal";
import { Placeholder } from "./Placeholder";
import { Row } from "./Row";
import { Slider } from "./Slider";
import { Tabs } from "./Tabs";
import { Text } from "./Text";
import { TextField } from "./TextField";
import { Video } from "./Video";

/** The shadcn/ui implementation of the A2UI `ComponentKit` contract. */
export const shadcnKit: ComponentKit = {
  Text,
  Image,
  Icon,
  Video,
  AudioPlayer,
  Row,
  Column,
  List,
  Card,
  Tabs,
  Modal,
  Divider,
  Button,
  TextField,
  CheckBox,
  ChoicePicker,
  Slider,
  DateTimeInput,
  Placeholder,
};

export {
  Text,
  Image,
  Icon,
  Video,
  AudioPlayer,
  Row,
  Column,
  List,
  Card,
  Tabs,
  Modal,
  Divider,
  Button,
  TextField,
  CheckBox,
  ChoicePicker,
  Slider,
  DateTimeInput,
  Placeholder,
};
