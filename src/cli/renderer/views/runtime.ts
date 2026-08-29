import type { CustomTopic } from '@cdot65/prisma-airs-sdk';
import type { ApiKeyInfo, CustomerAppInfo, SecurityProfileInfo } from '../../../airs/types.js';
import {
  renderApiKeyDetail,
  renderApiKeyList,
  renderCustomerAppDetail,
  renderCustomerAppList,
  renderProfileDetail,
  renderProfileList,
  renderTopicDetail,
  renderTopicList,
} from '../runtime.js';
import type { ResourceView } from '../view.js';

export const profilesView: ResourceView<SecurityProfileInfo> = {
  name: 'profiles',
  columns: [
    { key: 'profileId', label: 'ID' },
    { key: 'profileName', label: 'Name' },
    { key: 'active', label: 'Active' },
    { key: 'revision', label: 'Revision' },
  ],
  pretty: {
    list: (items) => renderProfileList(items, 'pretty'),
    detail: renderProfileDetail,
  },
};

export const apiKeysView: ResourceView<ApiKeyInfo> = {
  name: 'API keys',
  columns: [
    { key: 'id', label: 'ID' },
    { key: 'name', label: 'Name' },
    { key: 'last8', label: 'Last 8' },
    { key: 'expiresAt', label: 'Expires' },
  ],
  pretty: {
    list: (items) => renderApiKeyList(items, 'pretty'),
    detail: renderApiKeyDetail,
  },
};

export const customerAppsView: ResourceView<CustomerAppInfo> = {
  name: 'customer apps',
  columns: [
    { key: 'id', label: 'ID' },
    { key: 'name', label: 'Name' },
    { key: 'description', label: 'Description' },
  ],
  pretty: {
    list: (items) => renderCustomerAppList(items, 'pretty'),
    detail: renderCustomerAppDetail,
  },
};

export const topicsView: ResourceView<CustomTopic> = {
  name: 'topics',
  columns: [
    { key: 'topic_id', label: 'ID' },
    { key: 'topic_name', label: 'Name' },
    { key: 'revision', label: 'Revision' },
    { key: 'description', label: 'Description' },
  ],
  structured: (topic) => ({
    topicId: topic.topic_id,
    topicName: topic.topic_name,
    revision: topic.revision,
    active: topic.active,
    description: topic.description,
    examples: topic.examples,
    createdBy: topic.created_by,
    updatedBy: topic.updated_by,
    lastModifiedTs: topic.last_modified_ts,
    createdTs: topic.created_ts,
  }),
  pretty: {
    list: (items) => renderTopicList(items, 'pretty'),
    detail: renderTopicDetail,
  },
};
