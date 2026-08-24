ALTER TABLE collaboration_participants
ADD COLUMN media_transport TEXT;

ALTER TABLE collaboration_participants
ADD COLUMN contribution_endpoint_url TEXT;

ALTER TABLE collaboration_participants
ADD COLUMN return_endpoint_url TEXT;
