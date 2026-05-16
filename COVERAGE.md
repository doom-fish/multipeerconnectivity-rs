# MultipeerConnectivity coverage audit

Audit source: `$(xcrun --sdk macosx --show-sdk-path)/System/Library/Frameworks/MultipeerConnectivity.framework/Versions/A/Headers`

## MCPeerID

| API | Status | Notes |
| --- | --- | --- |
| `-initWithDisplayName:` | ✅ implemented | `PeerId::new` |
| `displayName` | ✅ implemented | `PeerId::display_name` |
| `NSCopying` semantics | ✅ implemented | `Clone` retains the underlying peer ID |
| `NSSecureCoding` semantics | ✅ implemented | `PeerId::archived_data` / `PeerId::from_archived_data` |

## MCSession

| API | Status | Notes |
| --- | --- | --- |
| `-initWithPeer:` | ✅ implemented | `Session::new` |
| `-initWithPeer:securityIdentity:encryptionPreference:` | ✅ implemented | `Session::with_security_identity` / `Session::with_security_identity_items` |
| `sendData:toPeers:withMode:error:` | ✅ implemented | `Session::send` |
| `disconnect` | ✅ implemented | `Session::disconnect` |
| `sendResourceAtURL:withName:toPeer:withCompletionHandler:` | ✅ implemented | `Session::send_resource` / `Session::send_resource_with_completion` |
| `startStreamWithName:toPeer:error:` | ✅ implemented | `Session::start_stream` |
| `delegate` | ✅ implemented | `Session::set_delegate` / `Session::set_callbacks` / `clear_delegate` |
| `myPeerID` | ✅ implemented | `Session::my_peer_id` |
| `securityIdentity` | ✅ implemented | `Session::security_identity` |
| `encryptionPreference` | ✅ implemented | `Session::encryption_preference` |
| `connectedPeers` | ✅ implemented | `Session::connected_peers` |
| `kMCSessionMinimumNumberOfPeers` | ✅ implemented | `session_minimum_number_of_peers()` |
| `kMCSessionMaximumNumberOfPeers` | ✅ implemented | `session_maximum_number_of_peers()` |
| `session:peer:didChangeState:` | ✅ implemented | `SessionDelegate::on_state` |
| `session:didReceiveData:fromPeer:` | ✅ implemented | `SessionDelegate::on_data` |
| `session:didReceiveStream:withName:fromPeer:` | ✅ implemented | `SessionDelegate::on_stream` + `InputStream` |
| `session:didStartReceivingResourceWithName:fromPeer:withProgress:` | ✅ implemented | `SessionDelegate::on_resource_started` + `ResourceTransfer` |
| `session:didFinishReceivingResourceWithName:fromPeer:atURL:withError:` | ✅ implemented | `SessionDelegate::on_resource_finished` |
| `session:didReceiveCertificate:fromPeer:certificateHandler:` | ✅ implemented | `SessionDelegate::on_certificate` |
| `nearbyConnectionDataForPeer:withCompletionHandler:` | ✅ implemented | `Session::nearby_connection_data_for_peer` |
| `connectPeer:withNearbyConnectionData:` | ✅ implemented | `Session::connect_peer` |
| `cancelConnectPeer:` | ✅ implemented | `Session::cancel_connect_peer` |

## MCNearbyServiceAdvertiser

| API | Status | Notes |
| --- | --- | --- |
| `-initWithPeer:discoveryInfo:serviceType:` | ✅ implemented | `NearbyServiceAdvertiser::new` |
| `startAdvertisingPeer` | ✅ implemented | `NearbyServiceAdvertiser::start_advertising_peer` |
| `stopAdvertisingPeer` | ✅ implemented | `NearbyServiceAdvertiser::stop_advertising_peer` |
| `delegate` | ✅ implemented | `set_delegate` / `set_callbacks` / `clear_delegate` |
| `myPeerID` | ✅ implemented | `NearbyServiceAdvertiser::my_peer_id` |
| `discoveryInfo` | ✅ implemented | `NearbyServiceAdvertiser::discovery_info` |
| `serviceType` | ✅ implemented | `NearbyServiceAdvertiser::service_type` |
| `advertiser:didReceiveInvitationFromPeer:withContext:invitationHandler:` | ✅ implemented | `NearbyServiceAdvertiserDelegate::on_invitation` |
| `advertiser:didNotStartAdvertisingPeer:` | ✅ implemented | `NearbyServiceAdvertiserDelegate::on_error` |

## MCNearbyServiceBrowser

| API | Status | Notes |
| --- | --- | --- |
| `-initWithPeer:serviceType:` | ✅ implemented | `NearbyServiceBrowser::new` |
| `startBrowsingForPeers` | ✅ implemented | `NearbyServiceBrowser::start_browsing_for_peers` |
| `stopBrowsingForPeers` | ✅ implemented | `NearbyServiceBrowser::stop_browsing_for_peers` |
| `invitePeer:toSession:withContext:timeout:` | ✅ implemented | `NearbyServiceBrowser::invite_peer` |
| `delegate` | ✅ implemented | `set_delegate` / `set_callbacks` / `clear_delegate` |
| `myPeerID` | ✅ implemented | `NearbyServiceBrowser::my_peer_id` |
| `serviceType` | ✅ implemented | `NearbyServiceBrowser::service_type` |
| `browser:foundPeer:withDiscoveryInfo:` | ✅ implemented | `NearbyServiceBrowserDelegate::on_found` |
| `browser:lostPeer:` | ✅ implemented | `NearbyServiceBrowserDelegate::on_lost` |
| `browser:didNotStartBrowsingForPeers:` | ✅ implemented | `NearbyServiceBrowserDelegate::on_error` |

## MCAdvertiserAssistant

| API | Status | Notes |
| --- | --- | --- |
| `-initWithServiceType:discoveryInfo:session:` | ✅ implemented | `AdvertiserAssistant::new` |
| `start` | ✅ implemented | `AdvertiserAssistant::start` |
| `stop` | ✅ implemented | `AdvertiserAssistant::stop` |
| `delegate` | ✅ implemented | `set_callbacks` / `clear_delegate` |
| `session` | ✅ implemented | `AdvertiserAssistant::session` |
| `discoveryInfo` | ✅ implemented | `AdvertiserAssistant::discovery_info` |
| `serviceType` | ✅ implemented | `AdvertiserAssistant::service_type` |
| `advertiserAssistantWillPresentInvitation:` | ✅ implemented | `AdvertiserAssistantDelegate::on_will_present_invitation` |
| `advertiserAssistantDidDismissInvitation:` | ✅ implemented | `AdvertiserAssistantDelegate::on_did_dismiss_invitation` |

## MCBrowserViewController

| API | Status | Notes |
| --- | --- | --- |
| `-initWithServiceType:session:` | ✅ implemented | `BrowserViewController::new_with_service_type` |
| `-initWithBrowser:session:` | ✅ implemented | `BrowserViewController::new_with_browser` |
| `delegate` | ✅ implemented | `set_callbacks` / `clear_delegate` |
| `browser` | ✅ implemented | `BrowserViewController::browser` |
| `session` | ✅ implemented | `BrowserViewController::session` |
| `minimumNumberOfPeers` | ✅ implemented | getter + setter |
| `maximumNumberOfPeers` | ✅ implemented | getter + setter |
| `browserViewControllerDidFinish:` | ✅ implemented | `BrowserViewControllerDelegate::on_finish` |
| `browserViewControllerWasCancelled:` | ✅ implemented | `BrowserViewControllerDelegate::on_cancel` |
| `browserViewController:shouldPresentNearbyPeer:withDiscoveryInfo:` | ✅ implemented | `BrowserViewControllerDelegate::should_present_peer` |

## MCError

| API | Status | Notes |
| --- | --- | --- |
| `MCErrorDomain` | ✅ implemented | `mc_error_domain()` |
| `MCErrorUnknown` | ✅ implemented | `MCErrorCode::Unknown` |
| `MCErrorNotConnected` | ✅ implemented | `MCErrorCode::NotConnected` |
| `MCErrorInvalidParameter` | ✅ implemented | `MCErrorCode::InvalidParameter` |
| `MCErrorUnsupported` | ✅ implemented | `MCErrorCode::Unsupported` |
| `MCErrorTimedOut` | ✅ implemented | `MCErrorCode::TimedOut` |
| `MCErrorCancelled` | ✅ implemented | `MCErrorCode::Cancelled` |
| `MCErrorUnavailable` | ✅ implemented | `MCErrorCode::Unavailable` |
